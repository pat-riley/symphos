//! GPU-only visualizer library. The host owns audio capture, UI, signal mapping,
//! persistence and clocks. Instances own simulation/history; pipelines are shared.
//! No UI dependency and no CPU particle readback in the interactive render path.
#[cfg(test)]
mod background_tests;
pub mod cymatics;
pub mod cymatics_gpu;
mod gpu;
pub mod particle_waves;
pub use gpu::{Library, OrbitalGpu};

/// Aligned shader ABI; each row is a WGSL vec4. Explicit rows avoid Rust/WGSL
/// padding ambiguity. CPU settings and GPU runtime state are deliberately separate.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Frame {
    pub clock: [f32; 4],  // dt, time, count, reset
    pub flock: [f32; 4],  // cohesion, alignment, separation, speed
    pub forces: [f32; 4], // attraction, orbit, repulsion, noise
    pub orbs: [f32; 4],   // count, spacing, radius, motion
    pub look: [f32; 4],   // size pixels, streak, brightness, hue
    pub light: [f32; 4],  // lighting, bloom, exposure, trail half-life
    pub view: [f32; 4],   // aspect, zoom, pan x, pan y
    pub axis_x: [f32; 4],
    pub axis_y: [f32; 4],
    pub axis_z: [f32; 4],
    pub audio: [f32; 4],  // energy, audio force, audio light, boundary radius
    pub output: [f32; 4], // width, height, output is sRGB, clear history
    pub particle_style: [f32; 4], // shape, size variation, aspect, softness
    pub particle_detail: [f32; 4], // ring width, rotation radians, spin, rotation variation
    pub particle_color: [f32; 4], // palette, driver, spread, saturation
    pub particle_light: [f32; 4], // opacity, brightness variation, color cycle, orb brightness
    pub color_start: [f32; 4], // custom gradient sRGB
    pub color_end: [f32; 4],
    pub orb_style: [f32; 4],    // style, visual size, aspect, opacity
    pub orb_shape: [f32; 4],    // size variation, pulse depth/rate, audio size
    pub orb_material: [f32; 4], // ambient, specular, roughness, rim
    pub orb_surface: [f32; 4],  // rim power, detail density, line width, rotation
    pub orb_light: [f32; 4],    // light azimuth/elevation, surface spin/tilt
    pub orb_color: [f32; 4],    // color mode, palette, offset, spread
    pub orb_tint: [f32; 4],     // saturation, cycle, surface gradient, reserved
    pub orb_start: [f32; 4],    // sRGB gradient start / solid color
    pub orb_end: [f32; 4],
    pub camera: [f32; 4],      // distance, focal scale, orthographic, reserved
    pub behavior: [f32; 4],    // neighborhood reach, rear awareness, personal space, agility
    pub dynamics: [f32; 4],    // drag, minimum speed, center pull, plane confinement
    pub field: [f32; 4],       // vertical drift, turbulence scale/speed, boundary return
    pub circulation: [f32; 4], // orbit tilt, reserved
    pub paths: [f32; 4],       // enabled, tracer count, seconds, width pixels
    pub path_style: [f32; 4],  // opacity, fade exponent, reserved
    pub path_clock: [f32; 4],  // newest sample, valid sample count, record flag, particle stride
    pub orb_field: [f32; 4],   // enabled, surface clearance, reach, repulsion
    pub orb_barrier: [f32; 4], // circulation, falloff, solid surface, field illumination
    pub orb_optics: [f32; 4],  // core occlusion, lens strength/reach, dispersion
    pub orb_corona: [f32; 4],  // halo brightness/width, ray brightness/length
    pub orb_flare: [f32; 4],   // ray density, drift, live burst, burst size
    pub orb_colors: [[f32; 4]; 5], // individual sRGB colors
}

/// Bound VRAM/fill-rate at Retina resolutions. Rendering quality is per component.
/// The target frame rate is a host/device preference, never simulation time.
pub fn target_size(width: f32, height: f32, scale: f32) -> [u32; 2] {
    let w = width.max(1.0);
    let h = height.max(1.0);
    let factor = scale.clamp(0.25, 1.0).min(1920.0 / w.max(h));
    [
        (w * factor).round().max(1.0) as u32,
        (h * factor).round().max(1.0) as u32,
    ]
}
pub fn particle_count(quality: u32) -> u32 {
    match quality {
        0 => 16_384,
        2 => 65_536,
        3 => 131_072,
        _ => 32_768,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shaders_validate_without_a_gpu() {
        for code in [
            include_str!("cymatics.wgsl"),
            include_str!("particle_waves.wgsl"),
            include_str!("spectral_bloom_feedback.wgsl"),
            include_str!("particle_waves_blit.wgsl"),
            include_str!("cymatics_blit.wgsl"),
            concat!(
                include_str!("orbital.wgsl"),
                "\n",
                include_str!("optics.wgsl")
            ),
            concat!(include_str!("post.wgsl"), "\n", include_str!("optics.wgsl")),
            concat!(
                include_str!("paths.wgsl"),
                "\n",
                include_str!("optics.wgsl")
            ),
        ] {
            let module = naga::front::wgsl::parse_str(code).unwrap();
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap();
        }
    }
    #[test]
    fn quality_and_retina_budget_are_bounded() {
        assert_eq!(particle_count(1), 32768);
        assert_eq!(target_size(3840.0, 2160.0, 1.0), [1920, 1080]);
        assert_eq!(target_size(0.0, 0.0, 0.5), [1, 1]);
    }
    /// Real GPU validation, including shader entry points, buffer layouts, resize,
    /// pause and independent instances. Explicitly ignored on GPU-less CI hosts.
    #[test]
    #[ignore = "requires a Metal/Vulkan/DX12 GPU"]
    fn orbital_gpu_validation_and_timing() {
        pollster::block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("GPU adapter");
            eprintln!("Adapter: {:?}", adapter.get_info());
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .unwrap();
            let errors = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let lib = Library::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            let mut frame = Frame {
                clock: [1.0 / 60.0, 0.0, 0.0, 0.0],
                flock: [0.8, 1.1, 0.8, 1.8],
                forces: [1.0, 1.8, 3.5, 0.45],
                orbs: [3.0, 1.3, 0.18, 0.18],
                look: [1.5, 0.9, 0.7, 0.05],
                light: [1.0, 0.7, 1.0, 0.09],
                view: [16.0 / 9.0, 1.0, 0.0, 0.0],
                axis_x: [1.0, 0.0, 0.0, 0.0],
                axis_y: [0.0, 1.0, 0.0, 0.0],
                axis_z: [0.0, 0.0, 1.0, 0.0],
                audio: [0.4, 0.65, 0.7, 4.5],
                output: [0.0; 4],
                particle_style: [0.0, 0.0, 1.0, 0.5],
                particle_detail: [0.3, 0.0, 0.0, 0.0],
                particle_color: [0.0, 0.0, 0.35, 1.0],
                particle_light: [1.0, 0.0, 0.0, 1.0],
                color_start: [0.0, 0.8, 1.0, 1.0],
                color_end: [1.0, 0.1, 0.5, 1.0],
                orb_style: [0.0, 1.0, 1.0, 1.0],
                orb_shape: [0.0, 0.0, 0.5, 0.4],
                orb_material: [0.5, 0.0, 0.4, 3.0],
                orb_surface: [3.0, 12.0, 0.12, 0.0],
                orb_light: [-26.565_f32.to_radians(), 28.22_f32.to_radians(), 0.0, 0.0],
                orb_color: [0.0, 2.0, 0.0, 1.0],
                orb_tint: [1.0, 0.0, 0.0, 0.0],
                orb_start: [0.0, 0.8, 1.0, 1.0],
                orb_end: [1.0, 0.1, 0.5, 1.0],
                camera: [8.5, 2.0, 0.0, 0.0],
                behavior: [1.75, 1.0, 0.6, 40.0],
                dynamics: [0.12, 0.0, 0.0, 0.0],
                field: [0.0, 1.0, 1.0, 3.0],
                circulation: [1.0, 0.0, 0.0, 0.0],
                orb_field: [0.0, 0.08, 0.8, 12.0],
                orb_barrier: [2.0, 1.5, 1.0, 0.0],
                orb_optics: [0.0, 0.0, 2.5, 0.0],
                orb_corona: [0.0, 0.18, 0.0, 2.0],
                orb_flare: [48.0, 0.14, 0.0, 0.2],
                paths: [0.0, 128.0, 2.0, 1.0],
                path_style: [0.5, 1.5, 0.0, 0.0],
                path_clock: [0.0; 4],
                orb_colors: [
                    [1.0, 0.0, 0.0, 1.0],
                    [0.0, 1.0, 0.0, 1.0],
                    [0.0, 0.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0, 1.0],
                ],
            };
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("test output"),
                size: wgpu::Extent3d {
                    width: 1280,
                    height: 720,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = target.create_view(&Default::default());
            for quality in [1, 2, 3] {
                let mut gpu = OrbitalGpu::new(&device, &lib, particle_count(quality), [1280, 720]);
                let start = std::time::Instant::now();
                for i in 0..120 {
                    frame.clock[1] = i as f32 / 60.0;
                    let mut encoder = device.create_command_encoder(&Default::default());
                    gpu.prepare(&device, &queue, &mut encoder, &lib, frame, [1280, 720]);
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("test composite"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            ..Default::default()
                        });
                        gpu.paint(&lib, &mut pass);
                    }
                    queue.submit([encoder.finish()]);
                    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                }
                eprintln!(
                    "{} particles at 1280×720: {:.2} ms/frame (CPU submission + GPU, synchronized)",
                    particle_count(quality),
                    start.elapsed().as_secs_f64() * 1000.0 / 120.0
                );
                if quality == 1 {
                    // Accelerated one-minute soak: no window, no sleeps, real GPU simulation.
                    for i in 120..3600 {
                        frame.clock[1] = i as f32 / 60.0;
                        let mut encoder = device.create_command_encoder(&Default::default());
                        gpu.prepare(&device, &queue, &mut encoder, &lib, frame, [1280, 720]);
                        queue.submit([encoder.finish()]);
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    }
                    assert!(
                        gpu.verify_cloud(&device, &queue) > 64,
                        "default flock retains volume"
                    );
                }
                // Validate actual image output, not merely successful submission.
                let readback = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("test readback"),
                    size: 1280 * 720 * 4,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut copy = device.create_command_encoder(&Default::default());
                copy.copy_texture_to_buffer(
                    target.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(1280 * 4),
                            rows_per_image: Some(720),
                        },
                    },
                    target.size(),
                );
                queue.submit([copy.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                readback
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let pixels = readback.slice(..).get_mapped_range().unwrap();
                let lit = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[0].max(p[1]).max(p[2]) > 40)
                    .count();
                assert!(lit > 1000, "render must contain visible particles and orbs");
                assert!(lit < 1280 * 720 * 9 / 10, "render must retain dark space");
                drop(pixels);
                readback.unmap();
                if quality == 1 {
                    // A paused simulation must redraw when appearance changes. Read back
                    // real pixels to catch ignored uniforms, ABI drift and inert controls.
                    let mut path_frames = 0;
                    let mut render = |style: Frame| {
                        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                        let mut encoder = device.create_command_encoder(&Default::default());
                        gpu.prepare(&device, &queue, &mut encoder, &lib, style, [1280, 720]);
                        {
                            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                ..Default::default()
                            });
                            gpu.paint(&lib, &mut pass);
                        }
                        encoder.copy_texture_to_buffer(
                            target.as_image_copy(),
                            wgpu::TexelCopyBufferInfo {
                                buffer: &readback,
                                layout: wgpu::TexelCopyBufferLayout {
                                    offset: 0,
                                    bytes_per_row: Some(1280 * 4),
                                    rows_per_image: Some(720),
                                },
                            },
                            target.size(),
                        );
                        queue.submit([encoder.finish()]);
                        let (tx, rx) = std::sync::mpsc::channel();
                        readback
                            .slice(..)
                            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        rx.recv().unwrap().unwrap();
                        if let Some(error) = pollster::block_on(validation.pop()) {
                            panic!("{error}");
                        }
                        if style.paths[0] > 0.5 && style.clock[0] > 0.0 {
                            path_frames += 1;
                            if path_frames == 160 {
                                gpu.verify_paths(&device, &queue);
                            }
                        }
                        let bytes = readback.slice(..).get_mapped_range().unwrap().to_vec();
                        readback.unmap();
                        bytes
                    };
                    let mut style = frame;
                    style.clock[0] = 0.0;
                    style.look[0] = 4.0;
                    style.look[1] = 0.0;
                    style.light[1] = 0.0;
                    style.light[3] = 0.0;
                    style.particle_light[3] = 0.0;
                    style.particle_color[2] = 1.0;
                    let mut shapes = std::collections::HashSet::new();
                    let checksum = |bytes: &[u8]| {
                        use std::hash::{Hash, Hasher};
                        let mut h = std::collections::hash_map::DefaultHasher::new();
                        bytes.hash(&mut h);
                        h.finish()
                    };
                    for shape in 0..7 {
                        style.particle_style[0] = shape as f32;
                        let pixels = render(style);
                        assert!(
                            pixels
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .any(|p| p[0].max(p[1]).max(p[2]) > 10),
                            "shape {shape} must render"
                        );
                        assert!(
                            shapes.insert(checksum(&pixels)),
                            "shape {shape} must be distinct"
                        );
                    }
                    let arrow = checksum(&render(style));
                    style.particle_detail[1] = 1.7;
                    style.particle_detail[2] = 2.0;
                    style.particle_detail[3] = 1.0;
                    assert_eq!(
                        arrow,
                        checksum(&render(style)),
                        "arrows must stay aligned to velocity despite decorative spin"
                    );
                    style.particle_detail = frame.particle_detail;
                    let mut palettes = std::collections::HashSet::new();
                    for palette in 0..8 {
                        style.particle_color[0] = palette as f32;
                        assert!(
                            palettes.insert(checksum(&render(style))),
                            "palette {palette} must be distinct"
                        );
                    }
                    let mut drivers = std::collections::HashSet::new();
                    for driver in 0..5 {
                        style.particle_color[1] = driver as f32;
                        assert!(
                            drivers.insert(checksum(&render(style))),
                            "color driver {driver} must be distinct"
                        );
                    }
                    let before = checksum(&render(style));
                    style.color_start = [1.0, 0.0, 0.0, 1.0];
                    assert_ne!(
                        before,
                        checksum(&render(style)),
                        "custom swatches must reach the GPU"
                    );
                    style.particle_light[0] = 0.0;
                    assert!(
                        render(style)
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0),
                        "zero particle opacity and orb brightness must produce black"
                    );
                    eprintln!(
                        "Appearance: seven shapes, eight palettes, five color drivers, custom swatches and paused edits verified from GPU pixels"
                    );
                    style.particle_light[3] = 1.0;
                    style.orb_style[1] = 2.5;
                    style.orb_color[0] = 1.0;
                    let mut styles = std::collections::HashSet::new();
                    for orb_style in 0..9 {
                        style.orb_style[0] = orb_style as f32;
                        let pixels = render(style);
                        assert!(
                            pixels
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .any(|p| p[0].max(p[1]).max(p[2]) > 10),
                            "orb style {orb_style} must render"
                        );
                        assert!(
                            styles.insert(checksum(&pixels)),
                            "orb style {orb_style} must be distinct"
                        );
                    }
                    let mut illuminated = style;
                    illuminated.particle_light[0] = 1.0;
                    let unlit = checksum(&render(illuminated));
                    illuminated.orb_barrier[3] = 2.5;
                    assert_ne!(
                        unlit,
                        checksum(&render(illuminated)),
                        "field illumination must brighten nearby particles"
                    );
                    let mut optics = style;
                    optics.orb_style[0] = 7.0;
                    optics.orb_optics = [1.0, 0.0, 2.8, 0.0];
                    let untreated = checksum(&render(optics));
                    optics.orb_optics[1] = 0.5;
                    let lens = checksum(&render(optics));
                    assert_ne!(untreated, lens, "lens must warp visible orb edges");
                    optics.orb_optics[3] = 0.5;
                    assert_ne!(
                        lens,
                        checksum(&render(optics)),
                        "dispersion must alter lens color fringes"
                    );
                    optics.orb_corona = [0.7, 0.15, 0.4, 2.0];
                    let corona = checksum(&render(optics));
                    optics.orb_flare[2] = 1.0;
                    assert_ne!(
                        corona,
                        checksum(&render(optics)),
                        "bursts must brighten corona and expand the orb"
                    );
                    if let Ok(directory) = std::env::var("SYMPHOS_GPU_PREVIEW_DIR") {
                        // Portable proof image; no image codec in the renderer library.
                        std::fs::create_dir_all(&directory).unwrap();
                        optics.particle_light[0] = 1.0;
                        optics.look = [0.9, 0.8, 0.7, 0.05];
                        optics.orb_style[1] = 2.8;
                        optics.orb_style[0] = 7.0;
                        optics.orb_flare[2] = 0.0;
                        optics.orb_material = [0.025, 0.0, 0.4, 2.5];
                        optics.orb_surface[0] = 5.0;
                        optics.orb_field = [1.0, 0.05, 1.2, 18.0];
                        optics.orb_barrier = [3.0, 1.5, 1.0, 2.5];
                        optics.particle_light[0] = 0.45;
                        optics.flock = [0.45, 1.4, 0.8, 1.8];
                        optics.forces = [1.2, 2.3, 3.5, 0.35];
                        optics.orb_color = [1.0, 4.0, 0.0, 0.75];
                        optics.particle_color = [4.0, 4.0, 0.75, 1.0];
                        for (name, solar) in [("lensing-swarm", false), ("solar-ribbons", true)] {
                            if solar {
                                optics.orb_style[0] = 8.0;
                                optics.orb_color[1] = 5.0;
                                optics.particle_color[0] = 5.0;
                                optics.orb_corona = [1.0, 0.3, 0.65, 2.5];
                            }
                            optics.clock[0] = 1.0 / 60.0;
                            for _ in 0..240 {
                                optics.clock[1] += optics.clock[0];
                                render(optics);
                            }
                            optics.clock[0] = 0.0;
                            let bytes = render(optics);
                            let mut ppm = b"P6\n1280 720\n255\n".to_vec();
                            for p in bytes.as_chunks::<4>().0 {
                                ppm.extend_from_slice(&p[..3]);
                            }
                            std::fs::write(
                                std::path::Path::new(&directory).join(format!("{name}.ppm")),
                                ppm,
                            )
                            .unwrap();
                        }
                    }
                    style.orb_style[0] = 0.0;
                    let mut colors = std::collections::HashSet::new();
                    for mode in 0..4 {
                        style.orb_color[0] = mode as f32;
                        assert!(
                            colors.insert(checksum(&render(style))),
                            "orb color mode {mode} must be distinct"
                        );
                    }
                    let before = checksum(&render(style));
                    style.orb_colors[0] = [0.0, 1.0, 1.0, 0.0];
                    assert_ne!(
                        before,
                        checksum(&render(style)),
                        "individual orb swatch must affect pixels"
                    );
                    style.orb_color[0] = 2.0;
                    let solid = checksum(&render(style));
                    style.particle_color[0] = 1.0;
                    style.look[3] = 0.8;
                    assert_eq!(
                        solid,
                        checksum(&render(style)),
                        "independent orb colors must ignore particle palette edits"
                    );
                    style.orb_start = [1.0, 0.2, 0.0, 1.0];
                    assert_ne!(
                        solid,
                        checksum(&render(style)),
                        "solid orb color must affect pixels"
                    );
                    style.orb_style[3] = 0.0;
                    assert!(
                        render(style)
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0),
                        "orb opacity must hide every style"
                    );
                    eprintln!(
                        "Orbs: nine styles, four color modes, individual/solid swatches and independent palette verified from GPU pixels"
                    );
                    style = frame;
                    style.clock[0] = 1.0 / 30.0;
                    style.paths = [1.0, 64.0, 2.0, 1.5];
                    style.path_style = [1.0, 1.5, 0.0, 0.0];
                    style.light[3] = 0.0;
                    style.particle_light[0] = 0.0;
                    style.orb_style[3] = 0.0;
                    let mut last = Vec::new();
                    for _ in 0..160 {
                        style.clock[1] += style.clock[0];
                        last = render(style);
                    }

                    assert!(
                        last.as_chunks::<4>()
                            .0
                            .iter()
                            .filter(|p| p[0].max(p[1]).max(p[2]) > 10)
                            .count()
                            > 100,
                        "world paths must render without particle heads or orbs after history wraps"
                    );
                    style.clock[0] = 0.0;
                    let stationary = checksum(&render(style));
                    style.view[1] = 1.2;
                    assert_ne!(
                        stationary,
                        checksum(&render(style)),
                        "paths must reproject while paused"
                    );
                    style.camera[2] = 1.0;
                    let orthographic = render(style);
                    assert!(
                        orthographic
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .any(|p| p[0].max(p[1]).max(p[2]) > 10),
                        "orthographic paths must remain visible"
                    );
                    style.paths[0] = 0.0;
                    assert!(
                        render(style)
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0),
                        "disabling paths must clear them immediately"
                    );
                    eprintln!(
                        "World paths: wraparound, independent visibility, paused reprojection, orthographic projection and disabling verified"
                    );
                    let mut extreme = frame;
                    extreme.clock[0] = 1.0 / 30.0;
                    extreme.flock = [4.0, 4.0, 4.0, 6.0];
                    extreme.forces = [5.0, -4.0, 8.0, 3.0];
                    extreme.behavior = [0.5, 0.0, 2.0, 60.0];
                    extreme.dynamics = [0.0, 2.0, 3.0, 5.0];
                    extreme.field = [2.0, 4.0, 3.0, 0.0];
                    extreme.circulation[0] = 2.0;
                    for _ in 0..120 {
                        extreme.clock[1] += extreme.clock[0];
                        render(extreme);
                    }
                    gpu.verify_cloud(&device, &queue);
                    eprintln!("Strong flocking forces remain finite and bounded");
                    OrbitalGpu::verify_surface_fields(&device, &queue, &lib, frame);
                    OrbitalGpu::verify_optical_depth(&device, &queue, &lib, frame);
                }
                let mut encoder = device.create_command_encoder(&Default::default());
                frame.clock[0] = 0.0;
                gpu.prepare(&device, &queue, &mut encoder, &lib, frame, [640, 360]);
                queue.submit([encoder.finish()]);
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                frame.clock[0] = 1.0 / 60.0;
            }
            if let Some(error) = errors.pop().await {
                panic!("{error}");
            }
        });
    }
}
