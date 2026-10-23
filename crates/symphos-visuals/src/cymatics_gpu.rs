//! Persistent per-pane GPU grains, shaded plate and nodal/interference rendering.
use crate::cymatics::GRID;
use wgpu::{self as w, util::DeviceExt};
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CymaticsFrame {
    pub clock: [f32; 4],     // dt, time, reset, seed
    pub geometry: [f32; 4],  // circle, aspect, mode, displacement
    pub motion: [f32; 4],    // migration, friction, agitation, artistic turbulence
    pub grains: [f32; 4],    // size px, variation, opacity, glow
    pub recycling: [f32; 4], // enabled, edge threshold, central spawn radius, renewal rate
    pub lifecycle: [f32; 4], // maximum speed, reserved
    pub surface: [f32; 4],   // opacity, brightness, node width, contours
    pub light: [f32; 4],     // ambient, diffuse, specular, roughness
    pub direction: [f32; 4], // light xyz, grain style
    pub view: [f32; 4],      // aspect, zoom, pan x/y
    pub axis_x: [f32; 4],
    pub axis_y: [f32; 4],
    pub axis_z: [f32; 4],
    pub camera: [f32; 4], // distance, focal, orthographic, mean vibration energy
    pub gradient: [[f32; 4]; 8], // sRGB stops, matching the shared palette editor
    pub palette: [f32; 4], // count, cyclic, reserved
    pub plate_color: [f32; 4],
    pub color: [f32; 4],  // driver, spread, offset, contrast
    pub output: [f32; 4], // width,height,srgb,grain count
}
pub struct CymaticsLibrary {
    sim: w::ComputePipeline,
    plate: w::RenderPipeline,
    grains: w::RenderPipeline,
    composite: w::RenderPipeline,
    sim_layout: w::BindGroupLayout,
    draw_layout: w::BindGroupLayout,
    texture_layout: w::BindGroupLayout,
    sampler: w::Sampler,
    format: w::TextureFormat,
}
fn entry(
    binding: u32,
    kind: w::BufferBindingType,
    visibility: w::ShaderStages,
) -> w::BindGroupLayoutEntry {
    w::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: w::BindingType::Buffer {
            ty: kind,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
impl CymaticsLibrary {
    pub fn new(d: &w::Device, format: w::TextureFormat) -> Self {
        let sim_layout = d.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Cymatics compute"),
            entries: &[
                entry(0, w::BufferBindingType::Uniform, w::ShaderStages::COMPUTE),
                entry(
                    1,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::COMPUTE,
                ),
                entry(
                    2,
                    w::BufferBindingType::Storage { read_only: false },
                    w::ShaderStages::COMPUTE,
                ),
            ],
        });
        let draw_layout = d.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Cymatics render"),
            entries: &[
                entry(
                    0,
                    w::BufferBindingType::Uniform,
                    w::ShaderStages::VERTEX_FRAGMENT,
                ),
                entry(
                    1,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::VERTEX_FRAGMENT,
                ),
                entry(
                    2,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::VERTEX,
                ),
            ],
        });
        let texture_layout = d.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Cymatics composite"),
            entries: &[
                w::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Texture {
                        sample_type: w::TextureSampleType::Float { filterable: true },
                        view_dimension: w::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                w::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Sampler(w::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = d.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Cymatics"),
            source: w::ShaderSource::Wgsl(include_str!("cymatics.wgsl").into()),
        });
        let sl = d.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&sim_layout)],
            immediate_size: 0,
        });
        let dl = d.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&draw_layout)],
            immediate_size: 0,
        });
        let sim = d.create_compute_pipeline(&w::ComputePipelineDescriptor {
            label: Some("Cymatics grain step"),
            layout: Some(&sl),
            module: &shader,
            entry_point: Some("simulate"),
            compilation_options: Default::default(),
            cache: None,
        });
        let source = include_str!("cymatics.wgsl");
        let start = source.find("@compute").unwrap();
        let end = source.find("fn project").unwrap();
        let render_source = format!("{}{}", &source[..start], &source[end..])
            .replace("var<storage,read_write> grains", "var<storage,read> grains");
        let render_shader = d.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Cymatics drawing"),
            source: w::ShaderSource::Wgsl(render_source.into()),
        });
        let make = |vertex, fragment, depth_write| {
            d.create_render_pipeline(&w::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&dl),
                vertex: w::VertexState {
                    module: &render_shader,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(w::FragmentState {
                    module: &render_shader,
                    entry_point: Some(fragment),
                    targets: &[Some(w::ColorTargetState {
                        format,
                        blend: Some(w::BlendState::ALPHA_BLENDING),
                        write_mask: w::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: Some(w::DepthStencilState {
                    format: w::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(w::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let plate = make("plate_vertex", "plate_fragment", true);
        let grains = make("grain_vertex", "grain_fragment", false);
        let blit = d.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Cymatics blit"),
            source: w::ShaderSource::Wgsl(include_str!("cymatics_blit.wgsl").into()),
        });
        let pl = d.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&texture_layout)],
            immediate_size: 0,
        });
        let composite = d.create_render_pipeline(&w::RenderPipelineDescriptor {
            label: Some("Cymatics composite"),
            layout: Some(&pl),
            vertex: w::VertexState {
                module: &blit,
                entry_point: Some("vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(w::FragmentState {
                module: &blit,
                entry_point: Some("fragment"),
                targets: &[Some(w::ColorTargetState {
                    format,
                    blend: Some(w::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: w::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = d.create_sampler(&w::SamplerDescriptor {
            mag_filter: w::FilterMode::Linear,
            min_filter: w::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            sim,
            plate,
            grains,
            composite,
            sim_layout,
            draw_layout,
            texture_layout,
            sampler,
            format,
        }
    }
}
pub struct CymaticsGpu {
    uniform: w::Buffer,
    field: w::Buffer,
    pub particles: w::Buffer,
    sim: w::BindGroup,
    draw: w::BindGroup,
    texture: w::Texture,
    view: w::TextureView,
    depth: w::TextureView,
    blit: w::BindGroup,
    size: [u32; 2],
    pub count: u32,
    first: bool,
}
fn target(
    d: &w::Device,
    l: &CymaticsLibrary,
    size: [u32; 2],
) -> (w::Texture, w::TextureView, w::TextureView, w::BindGroup) {
    let descriptor = w::TextureDescriptor {
        label: Some("Cymatics target"),
        size: w::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: w::TextureDimension::D2,
        format: l.format,
        usage: w::TextureUsages::RENDER_ATTACHMENT
            | w::TextureUsages::TEXTURE_BINDING
            | w::TextureUsages::COPY_SRC,
        view_formats: &[],
    };
    let texture = d.create_texture(&descriptor);
    let view = texture.create_view(&Default::default());
    let depth = d
        .create_texture(&w::TextureDescriptor {
            format: w::TextureFormat::Depth32Float,
            usage: w::TextureUsages::RENDER_ATTACHMENT,
            ..descriptor
        })
        .create_view(&Default::default());
    let blit = d.create_bind_group(&w::BindGroupDescriptor {
        label: None,
        layout: &l.texture_layout,
        entries: &[
            w::BindGroupEntry {
                binding: 0,
                resource: w::BindingResource::TextureView(&view),
            },
            w::BindGroupEntry {
                binding: 1,
                resource: w::BindingResource::Sampler(&l.sampler),
            },
        ],
    });
    (texture, view, depth, blit)
}
impl CymaticsGpu {
    pub fn new(d: &w::Device, l: &CymaticsLibrary, count: u32, size: [u32; 2]) -> Self {
        let uniform = d.create_buffer_init(&w::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&CymaticsFrame::zero()),
            usage: w::BufferUsages::UNIFORM | w::BufferUsages::COPY_DST,
        });
        let field = d.create_buffer(&w::BufferDescriptor {
            label: Some("Cymatics modal field"),
            size: (GRID * GRID * 16) as u64,
            usage: w::BufferUsages::STORAGE | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let particles = d.create_buffer(&w::BufferDescriptor {
            label: Some("Cymatics grains"),
            size: count as u64 * 16,
            usage: w::BufferUsages::STORAGE | w::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let entries = [
            w::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            w::BindGroupEntry {
                binding: 1,
                resource: field.as_entire_binding(),
            },
            w::BindGroupEntry {
                binding: 2,
                resource: particles.as_entire_binding(),
            },
        ];
        let sim = d.create_bind_group(&w::BindGroupDescriptor {
            label: None,
            layout: &l.sim_layout,
            entries: &entries,
        });
        let draw = d.create_bind_group(&w::BindGroupDescriptor {
            label: None,
            layout: &l.draw_layout,
            entries: &entries,
        });
        let (texture, view, depth, blit) = target(d, l, size);
        Self {
            uniform,
            field,
            particles,
            sim,
            draw,
            texture,
            view,
            depth,
            blit,
            size,
            count,
            first: true,
        }
    }
    pub fn prepare(
        &mut self,
        d: &w::Device,
        e: &mut w::CommandEncoder,
        l: &CymaticsLibrary,
        mut frame: CymaticsFrame,
        field: &[[f32; 4]],
        size: [u32; 2],
    ) {
        if size != self.size {
            (self.texture, self.view, self.depth, self.blit) = target(d, l, size);
            self.size = size;
        }
        frame.output = [
            size[0] as f32,
            size[1] as f32,
            if l.format.is_srgb() { 1.0 } else { 0.0 },
            self.count as f32,
        ];
        if self.first {
            frame.clock[2] = 1.0;
            self.first = false;
        }
        // Queue::write_buffer runs before all command buffers in the next
        // submission. A later callback could therefore overwrite an earlier
        // callback's scatter flag. Encode uploads before their own dispatch so
        // batched preparations each see the intended uniform and modal field.
        let uniform = bytemuck::bytes_of(&frame);
        let field = bytemuck::cast_slice(field);
        let mut bytes = Vec::with_capacity(uniform.len() + field.len());
        bytes.extend_from_slice(uniform);
        bytes.extend_from_slice(field);
        let upload = d.create_buffer_init(&w::util::BufferInitDescriptor {
            label: Some("Cymatics ordered frame upload"),
            contents: &bytes,
            usage: w::BufferUsages::COPY_SRC,
        });
        e.copy_buffer_to_buffer(&upload, 0, &self.uniform, 0, uniform.len() as u64);
        e.copy_buffer_to_buffer(
            &upload,
            uniform.len() as u64,
            &self.field,
            0,
            field.len() as u64,
        );
        {
            let mut pass = e.begin_compute_pass(&w::ComputePassDescriptor::default());
            pass.set_pipeline(&l.sim);
            pass.set_bind_group(0, &self.sim, &[]);
            pass.dispatch_workgroups(self.count.div_ceil(128), 1, 1);
        }
        {
            let mut pass = e.begin_render_pass(&w::RenderPassDescriptor {
                label: Some("Cymatics offscreen"),
                color_attachments: &[Some(w::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: w::Operations {
                        load: w::LoadOp::Clear(w::Color::TRANSPARENT),
                        store: w::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(w::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(w::Operations {
                        load: w::LoadOp::Clear(1.0),
                        store: w::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.draw, &[]);
            pass.set_pipeline(&l.plate);
            pass.draw(0..((GRID - 1) * (GRID - 1) * 6) as u32, 0..1);
            if frame.geometry[2] < 0.5 {
                pass.set_pipeline(&l.grains);
                pass.draw(0..6, 0..self.count);
            }
        }
    }
    pub fn paint(&self, l: &CymaticsLibrary, pass: &mut w::RenderPass<'_>) {
        pass.set_pipeline(&l.composite);
        pass.set_bind_group(0, &self.blit, &[]);
        pass.draw(0..3, 0..1);
    }
}
impl CymaticsFrame {
    pub fn zero() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(d: &w::Device, q: &w::Queue, b: &w::Buffer, size: u64) -> Vec<u8> {
        let readback = d.create_buffer(&w::BufferDescriptor {
            label: None,
            size,
            usage: w::BufferUsages::MAP_READ | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut e = d.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(b, 0, &readback, 0, size);
        q.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(w::MapMode::Read, move |r| {
            tx.send(r).unwrap();
        });
        d.poll(w::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        readback.slice(..).get_mapped_range().unwrap().to_vec()
    }
    #[test]
    fn cymatics_gradient_uniform_layout_matches_wgsl() {
        assert_eq!(std::mem::size_of::<CymaticsFrame>(), 26 * 16);
        assert_eq!(std::mem::offset_of!(CymaticsFrame, gradient), 14 * 16);
        assert_eq!(std::mem::offset_of!(CymaticsFrame, palette), 22 * 16);
        assert_eq!(std::mem::offset_of!(CymaticsFrame, output), 25 * 16);
    }
    #[test]
    #[ignore = "requires a real compute GPU"]
    fn cymatics_multistop_and_cyclic_palettes_render_in_every_mode_without_scattering() {
        pollster::block_on(async {
            let instance = w::Instance::default();
            let adapter = instance.request_adapter(&Default::default()).await.unwrap();
            let (d, q) = adapter.request_device(&Default::default()).await.unwrap();
            let errors = d.push_error_scope(w::ErrorFilter::Validation);
            let l = CymaticsLibrary::new(&d, w::TextureFormat::Rgba8Unorm);
            let mut gpu = CymaticsGpu::new(&d, &l, 4096, [192, 128]);
            let mut frame = CymaticsFrame::zero();
            frame.geometry = [0.0, 1.0, 0.0, 0.0];
            frame.grains = [3.0, 0.0, 1.0, 0.0];
            frame.surface = [1.0, 1.0, 0.075, 0.0];
            frame.light = [1.0, 0.0, 0.0, 0.5];
            frame.direction = [0.0, 1.0, 0.0, 0.0];
            frame.view = [1.5, 1.0, 0.0, 0.0];
            frame.axis_x = [1.0, 0.0, 0.0, 0.0];
            frame.axis_y = [0.0, 0.0, -1.0, 0.0];
            frame.axis_z = [0.0, 1.0, 0.0, 0.0];
            frame.camera = [4.0, 2.4, 1.0, 0.5];
            let red = [1.0, 0.0, 0.0, 1.0];
            let green = [0.0, 1.0, 0.0, 1.0];
            let blue = [0.0, 0.0, 1.0, 1.0];
            let field = vec![[0.0; 4]; GRID * GRID];
            let mut initial = None;
            for mode in 0..4 {
                frame.geometry[2] = mode as f32;
                frame.surface[0] = if mode == 0 { 0.0 } else { 1.0 };
                // Matching endpoint colors make a two-endpoint shortcut fail:
                // the middle stop must actually reach grains and surfaces.
                for (count, cyclic, t, expected) in [
                    (2, false, 0.5, [0.5, 0.5, 0.0, 1.0]),
                    (3, false, 0.25, [0.5, 0.5, 0.0, 1.0]),
                    (3, false, 0.5, green),
                    (3, true, 0.5, [0.5, 0.5, 0.0, 1.0]),
                    (3, true, 1.0 / 3.0, green),
                    (3, true, 1.0, red),
                    (8, false, 6.0 / 7.0, blue),
                    (8, true, 6.0 / 8.0, blue),
                    (8, true, 1.0, red),
                ] {
                    frame.gradient = [red; 8];
                    frame.gradient[1] = green;
                    frame.gradient[6] = blue;
                    frame.palette = [count as f32, u8::from(cyclic) as f32, 0.0, 0.0];
                    frame.color = [0.0, 0.0, t, 1.0];
                    let pixels = d.create_buffer(&w::BufferDescriptor {
                        label: None,
                        size: 192 * 128 * 4,
                        usage: w::BufferUsages::COPY_DST | w::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    });
                    let mut e = d.create_command_encoder(&Default::default());
                    gpu.prepare(&d, &mut e, &l, frame, &field, [192, 128]);
                    e.copy_texture_to_buffer(
                        w::TexelCopyTextureInfo {
                            texture: &gpu.texture,
                            mip_level: 0,
                            origin: w::Origin3d::ZERO,
                            aspect: w::TextureAspect::All,
                        },
                        w::TexelCopyBufferInfo {
                            buffer: &pixels,
                            layout: w::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(192 * 4),
                                rows_per_image: Some(128),
                            },
                        },
                        w::Extent3d {
                            width: 192,
                            height: 128,
                            depth_or_array_layers: 1,
                        },
                    );
                    q.submit([e.finish()]);
                    let data = read(&d, &q, &pixels, 192 * 128 * 4);
                    let brightest = data.as_chunks::<4>().0.iter().max_by_key(|p| p[3]).unwrap();
                    assert!(brightest[3] > 250, "mode {mode} renders opaque pixels");
                    for c in 0..3 {
                        assert!(
                            (brightest[c] as f32 - expected[c] * 255.0).abs() < 3.0,
                            "mode {mode}, stops {count}, cyclic {cyclic}, t {t}: {brightest:?}, expected {expected:?}"
                        );
                    }
                    let particles = read(&d, &q, &gpu.particles, 4096 * 16);
                    if let Some(initial) = &initial {
                        assert_eq!(&particles, initial, "color changes preserve paused grains");
                    } else {
                        initial = Some(particles);
                    }
                }
            }
            assert!(errors.pop().await.is_none());
        });
    }
    #[test]
    #[ignore = "requires a real compute GPU"]
    fn cymatics_gpu_settles_renders_all_modes_and_handles_resize() {
        pollster::block_on(async {
            let instance = w::Instance::default();
            let adapter = instance.request_adapter(&Default::default()).await.unwrap();
            let (d, q) = adapter.request_device(&Default::default()).await.unwrap();
            eprintln!("Cymatics adapter: {:?}", adapter.get_info());
            let scope = d.push_error_scope(w::ErrorFilter::Validation);
            let l = CymaticsLibrary::new(&d, w::TextureFormat::Rgba8Unorm);
            let mut gpu = CymaticsGpu::new(&d, &l, 32768, [960, 640]);
            let mut frame = CymaticsFrame::zero();
            frame.geometry = [0.0, 1.0, 0.0, 0.0];
            frame.motion = [0.65, 7.0, 0.02, 0.0];
            frame.grains = [1.2, 0.5, 0.9, 0.2];
            frame.lifecycle = [0.7, 0.0, 0.0, 0.0];
            frame.surface = [1.0, 1.0, 0.075, 10.0];
            frame.light = [0.35, 0.8, 0.3, 0.45];
            frame.direction = [0.4, 0.8, 0.4, 0.0];
            frame.view = [1.5, 1.0, 0.0, 0.0];
            frame.axis_x = [1.0, 0.0, 0.0, 0.0];
            frame.axis_y = [0.0, 0.0, -1.0, 0.0];
            frame.axis_z = [0.0, 1.0, 0.0, 0.0];
            frame.camera = [4.0, 2.4, 1.0, 0.5];
            // Exercise the full eight-stop lookup in the render timing below.
            frame.gradient = std::array::from_fn(|i| {
                let t = i as f32 / 7.0;
                [0.9 + 0.1 * t, 0.8 + 0.2 * t, 0.6 + 0.35 * t, 1.0]
            });
            frame.palette = [8.0, 0.0, 0.0, 0.0];
            frame.plate_color = [0.02, 0.03, 0.04, 1.0];
            frame.color = [0.0, 0.7, 0.0, 1.0];
            let field: Vec<_> = crate::cymatics::grid()
                .map(|(x, y)| {
                    [
                        x.powi(2) as f32,
                        ((x * 3.0).sin() * (y * 3.0).sin()) as f32,
                        2.0 * x as f32,
                        0.0,
                    ]
                })
                .collect();
            let mut start_x = 0.0;
            let start = std::time::Instant::now();
            for i in 0..361 {
                frame.clock = [
                    if i == 0 { 0.0 } else { 1.0 / 60.0 },
                    i as f32 / 60.0,
                    0.0,
                    1.0,
                ];
                let mut e = d.create_command_encoder(&Default::default());
                gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
                // Two callbacks in one submission must preserve the initial
                // scatter, rather than drawing all grains at the origin.
                if i == 0 {
                    gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
                }
                q.submit([e.finish()]);
                if i == 0 {
                    let data = read(&d, &q, &gpu.particles, 32768 * 16);
                    let values: &[[f32; 4]] = bytemuck::cast_slice(&data);
                    start_x = values.iter().map(|v| v[0].abs()).sum::<f32>() / 32768.0;
                    assert!(
                        start_x > 0.4,
                        "grains scatter across the plate after batched preparation: {start_x}"
                    );
                }
            }
            d.poll(w::PollType::wait_indefinitely()).unwrap();
            eprintln!(
                "32K grains + surface, 960×640: {:.2} ms/frame",
                start.elapsed().as_secs_f64() * 1000.0 / 361.0
            );
            let data = read(&d, &q, &gpu.particles, 32768 * 16);
            let values: &[[f32; 4]] = bytemuck::cast_slice(&data);
            let end_x = values.iter().map(|v| v[0].abs()).sum::<f32>() / 32768.0;
            assert!(values.iter().flatten().all(|x| x.is_finite()));
            assert!(
                values
                    .iter()
                    .all(|p| p[0].abs() <= 1.0 && p[1].abs() <= 1.0)
            );
            assert!(
                end_x < start_x * 0.5,
                "sand settles toward node: {start_x} -> {end_x}"
            );
            let before_pause = data.clone();
            frame.clock[0] = 0.0;
            let mut e = d.create_command_encoder(&Default::default());
            gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
            q.submit([e.finish()]);
            assert_eq!(
                before_pause,
                read(&d, &q, &gpu.particles, 32768 * 16),
                "paused grains do not move"
            );
            // Real compute coverage: rectangular and circular rims recycle to
            // a dispersed central region; disabling, silence and pause retain grains.
            for circle in [0.0, 1.0] {
                for (enabled, energy, dt, should_recycle) in [
                    (1.0, 0.5, 1.0 / 120.0, true),
                    (0.0, 0.5, 1.0 / 120.0, false),
                    (1.0, 0.0, 1.0 / 120.0, false),
                    (1.0, 0.5, 0.0, false),
                ] {
                    frame.geometry = [circle, 1.7, 0.0, 0.0];
                    frame.motion = [0.0; 4];
                    frame.recycling = [enabled, 0.96, 0.35, 0.0];
                    frame.camera[3] = energy;
                    frame.clock = [0.0, 10.0, 1.0, 5.0];
                    let mut e = d.create_command_encoder(&Default::default());
                    gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
                    q.submit([e.finish()]);
                    let initial = read(&d, &q, &gpu.particles, 32768 * 16);
                    let initial: &[[f32; 4]] = bytemuck::cast_slice(&initial);
                    frame.clock = [dt, 10.0, 0.0, 5.0];
                    let mut e = d.create_command_encoder(&Default::default());
                    gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
                    q.submit([e.finish()]);
                    let result = read(&d, &q, &gpu.particles, 32768 * 16);
                    let result: &[[f32; 4]] = bytemuck::cast_slice(&result);
                    let mut recycled = Vec::new();
                    for (before, after) in initial.iter().zip(result) {
                        let edge = if circle > 0.5 {
                            before[0].hypot(before[1])
                        } else {
                            before[0].abs().max(before[1].abs())
                        };
                        if should_recycle && edge >= 0.96 {
                            assert!(after[0].hypot(after[1]) <= 0.35001);
                            assert_eq!(&after[2..], &[0.0, 0.0]);
                            recycled.push(after[0]);
                        } else if edge < 0.99 || dt == 0.0 {
                            assert_eq!(before, after, "only active rim grains recycle");
                        }
                        assert!(after.iter().all(|v| v.is_finite()));
                    }
                    if should_recycle {
                        assert!(recycled.len() > 100);
                        assert!(recycled.iter().any(|x| *x < -0.15));
                        assert!(recycled.iter().any(|x| *x > 0.15));
                    }
                }
            }
            // Artistic renewal works independently of the edge switch, but
            // cannot disturb a paused or silent plate.
            for (energy, dt, expected) in [
                (0.5, 1.0 / 120.0, true),
                (0.0, 1.0 / 120.0, false),
                (0.5, 0.0, false),
            ] {
                frame.geometry = [0.0, 1.0, 0.0, 0.0];
                frame.motion = [0.0; 4];
                frame.recycling = [0.0, 0.96, 0.2, 2.0];
                frame.camera[3] = energy;
                frame.clock = [0.0, 15.0, 1.0, 9.0];
                let mut e = d.create_command_encoder(&Default::default());
                gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
                q.submit([e.finish()]);
                let before = read(&d, &q, &gpu.particles, 32768 * 16);
                let before: &[[f32; 4]] = bytemuck::cast_slice(&before);
                frame.clock = [dt, 15.0, 0.0, 9.0];
                let mut e = d.create_command_encoder(&Default::default());
                gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
                q.submit([e.finish()]);
                let after = read(&d, &q, &gpu.particles, 32768 * 16);
                let after: &[[f32; 4]] = bytemuck::cast_slice(&after);
                let renewed: Vec<_> = before
                    .iter()
                    .zip(after)
                    .filter(|(a, b)| a[0].abs() < 0.9 && a[1].abs() < 0.9 && a != b)
                    .collect();
                if expected {
                    assert!(
                        renewed.len() > 200 && renewed.len() < 1000,
                        "renewal count: {}",
                        renewed.len()
                    );
                    assert!(
                        renewed.iter().all(|(_, b)| b[0].hypot(b[1]) <= 0.20001
                            && b[2] == 0.0
                            && b[3] == 0.0)
                    );
                } else {
                    assert!(renewed.is_empty());
                }
            }
            frame.lifecycle[0] = 0.1;
            frame.motion = [4.0, 0.0, 0.5, 1.0];
            frame.recycling = [0.0; 4];
            frame.clock = [1.0 / 30.0, 16.0, 0.0, 9.0];
            frame.camera[3] = 1.0;
            let mut e = d.create_command_encoder(&Default::default());
            gpu.prepare(&d, &mut e, &l, frame, &field, [960, 640]);
            q.submit([e.finish()]);
            let speeds = read(&d, &q, &gpu.particles, 32768 * 16);
            let speeds: &[[f32; 4]] = bytemuck::cast_slice(&speeds);
            assert!(speeds.iter().all(|p| p[2].hypot(p[3]) <= 0.10001));
            assert!(speeds.iter().any(|p| p[2].hypot(p[3]) > 0.05));
            for mode in 0..4 {
                frame.geometry = [1.0, 1.0, mode as f32, if mode == 0 { 0.0 } else { 0.1 }];
                frame.clock[2] = 1.0;
                let mut e = d.create_command_encoder(&Default::default());
                gpu.prepare(&d, &mut e, &l, frame, &field, [640, 480]);
                q.submit([e.finish()]);
                let pixels = d.create_buffer(&w::BufferDescriptor {
                    label: None,
                    size: 640 * 480 * 4,
                    usage: w::BufferUsages::COPY_DST | w::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let mut e = d.create_command_encoder(&Default::default());
                e.copy_texture_to_buffer(
                    w::TexelCopyTextureInfo {
                        texture: &gpu.texture,
                        mip_level: 0,
                        origin: w::Origin3d::ZERO,
                        aspect: w::TextureAspect::All,
                    },
                    w::TexelCopyBufferInfo {
                        buffer: &pixels,
                        layout: w::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(640 * 4),
                            rows_per_image: Some(480),
                        },
                    },
                    w::Extent3d {
                        width: 640,
                        height: 480,
                        depth_or_array_layers: 1,
                    },
                );
                q.submit([e.finish()]);
                let data = read(&d, &q, &pixels, 640 * 480 * 4);
                assert!(
                    data.as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8)
                        .count()
                        > 1000,
                    "mode {mode} produces an image"
                );
            }
            assert!(scope.pop().await.is_none());
        });
    }
}
