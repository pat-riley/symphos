use crate::Frame;
use wgpu::{self as w, util::DeviceExt};
const HDR: w::TextureFormat = w::TextureFormat::Rgba16Float;

/// Shared compiled pipelines. Created once per device, independently of scene state.
pub struct Library {
    layout: w::BindGroupLayout,
    post_layout: w::BindGroupLayout,
    path_layout: w::BindGroupLayout,
    render_layout: w::BindGroupLayout,
    paths: w::RenderPipeline,
    sampler: w::Sampler,
    clear: w::ComputePipeline,
    bin: w::ComputePipeline,
    simulate: w::ComputePipeline,
    particles: w::RenderPipeline,
    history: w::RenderPipeline,
    horizontal: w::RenderPipeline,
    vertical: w::RenderPipeline,
    composite: w::RenderPipeline,
    output_srgb: bool,
}
fn buffer_entry(
    binding: u32,
    ty: w::BufferBindingType,
    visibility: w::ShaderStages,
) -> w::BindGroupLayoutEntry {
    w::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: w::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn pipeline(
    device: &w::Device,
    layout: &w::PipelineLayout,
    shader: &w::ShaderModule,
    entry: &str,
    format: w::TextureFormat,
    blend: Option<w::BlendState>,
) -> w::RenderPipeline {
    device.create_render_pipeline(&w::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: w::VertexState {
            module: shader,
            entry_point: Some("vertex"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(w::FragmentState {
            module: shader,
            entry_point: Some(entry),
            targets: &[Some(w::ColorTargetState {
                format,
                blend,
                write_mask: w::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
impl Library {
    pub fn new(device: &w::Device, output: w::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Orbital simulation"),
            entries: &[
                buffer_entry(
                    0,
                    w::BufferBindingType::Uniform,
                    w::ShaderStages::VERTEX_FRAGMENT | w::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    1,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::VERTEX | w::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    2,
                    w::BufferBindingType::Storage { read_only: false },
                    w::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    3,
                    w::BufferBindingType::Storage { read_only: false },
                    w::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    4,
                    w::BufferBindingType::Storage { read_only: false },
                    w::ShaderStages::COMPUTE,
                ),
            ],
        });
        // Compute-only writable buffers must not enter the render-pass usage
        // scope, where trajectory rendering reads the same history buffer.
        let render_layout = device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Read-only particle rendering"),
            entries: &[
                buffer_entry(
                    0,
                    w::BufferBindingType::Uniform,
                    w::ShaderStages::VERTEX_FRAGMENT,
                ),
                buffer_entry(
                    1,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::VERTEX,
                ),
            ],
        });
        let path_layout = device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("World-space paths"),
            entries: &[
                buffer_entry(
                    0,
                    w::BufferBindingType::Uniform,
                    w::ShaderStages::VERTEX_FRAGMENT,
                ),
                buffer_entry(
                    1,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::VERTEX,
                ),
                buffer_entry(
                    2,
                    w::BufferBindingType::Storage { read_only: true },
                    w::ShaderStages::VERTEX,
                ),
            ],
        });
        let post_layout = device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("HDR compositing"),
            entries: &[
                buffer_entry(0, w::BufferBindingType::Uniform, w::ShaderStages::FRAGMENT),
                w::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Texture {
                        sample_type: w::TextureSampleType::Float { filterable: true },
                        view_dimension: w::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                w::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Texture {
                        sample_type: w::TextureSampleType::Float { filterable: true },
                        view_dimension: w::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                w::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Sampler(w::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Orbital GPU flocking"),
            source: w::ShaderSource::Wgsl(
                concat!(
                    include_str!("orbital.wgsl"),
                    "\n",
                    include_str!("optics.wgsl")
                )
                .into(),
            ),
        });
        let post = device.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("HDR post processing"),
            source: w::ShaderSource::Wgsl(
                concat!(include_str!("post.wgsl"), "\n", include_str!("optics.wgsl")).into(),
            ),
        });
        let pl = device.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: Some("Orbital"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let rp = device.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: Some("Particle rendering"),
            bind_group_layouts: &[Some(&render_layout)],
            immediate_size: 0,
        });
        let pp = device.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: Some("Post processing"),
            bind_group_layouts: &[Some(&post_layout)],
            immediate_size: 0,
        });
        let path_shader = device.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Particle trajectories"),
            source: w::ShaderSource::Wgsl(
                concat!(
                    include_str!("paths.wgsl"),
                    "\n",
                    include_str!("optics.wgsl")
                )
                .into(),
            ),
        });
        let path_pipeline_layout = device.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: Some("Trajectory rendering"),
            bind_group_layouts: &[Some(&path_layout)],
            immediate_size: 0,
        });
        let compute = |entry| {
            device.create_compute_pipeline(&w::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let additive = w::BlendComponent {
            src_factor: w::BlendFactor::One,
            dst_factor: w::BlendFactor::One,
            operation: w::BlendOperation::Add,
        };
        Self {
            paths: pipeline(
                device,
                &path_pipeline_layout,
                &path_shader,
                "fragment",
                HDR,
                Some(w::BlendState {
                    color: additive,
                    alpha: additive,
                }),
            ),
            path_layout,
            render_layout,
            clear: compute("clear_grid"),
            bin: compute("bin_particles"),
            simulate: compute("simulate"),
            particles: pipeline(
                device,
                &rp,
                &shader,
                "fragment",
                HDR,
                Some(w::BlendState {
                    color: additive,
                    alpha: additive,
                }),
            ),
            history: pipeline(device, &pp, &post, "history", HDR, None),
            horizontal: pipeline(device, &pp, &post, "horizontal", HDR, None),
            vertical: pipeline(device, &pp, &post, "vertical", HDR, None),
            composite: pipeline(device, &pp, &post, "composite", output, None),
            sampler: device.create_sampler(&w::SamplerDescriptor {
                label: Some("HDR linear clamp"),
                mag_filter: w::FilterMode::Linear,
                min_filter: w::FilterMode::Linear,
                ..Default::default()
            }),
            layout,
            post_layout,
            output_srgb: output.is_srgb(),
        }
    }
    fn post_group(
        &self,
        device: &w::Device,
        uniform: &w::Buffer,
        image: &w::TextureView,
        glow: &w::TextureView,
    ) -> w::BindGroup {
        device.create_bind_group(&w::BindGroupDescriptor {
            label: Some("HDR textures"),
            layout: &self.post_layout,
            entries: &[
                w::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                w::BindGroupEntry {
                    binding: 1,
                    resource: w::BindingResource::TextureView(image),
                },
                w::BindGroupEntry {
                    binding: 2,
                    resource: w::BindingResource::TextureView(glow),
                },
                w::BindGroupEntry {
                    binding: 3,
                    resource: w::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}
/// Reusable render-to-texture target. No window or UI handles cross this boundary.
struct Target {
    view: w::TextureView,
}
impl Target {
    fn new(device: &w::Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&w::TextureDescriptor {
            label: Some("Visualizer HDR target"),
            size: w::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: HDR,
            usage: w::TextureUsages::RENDER_ATTACHMENT | w::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Self {
            view: texture.create_view(&Default::default()),
        }
    }
}
struct Images {
    size: [u32; 2],
    history: [Target; 2],
    blur: [Target; 2],
    feedback: [w::BindGroup; 2],
    horizontal: [w::BindGroup; 2],
    vertical: w::BindGroup,
    composite: [w::BindGroup; 2],
}
impl Images {
    fn new(device: &w::Device, lib: &Library, uniform: &w::Buffer, size: [u32; 2]) -> Self {
        let history = std::array::from_fn(|_| Target::new(device, size));
        let blur = std::array::from_fn(|_| {
            Target::new(device, [(size[0] / 4).max(1), (size[1] / 4).max(1)])
        });
        Self {
            feedback: std::array::from_fn(|i| {
                lib.post_group(device, uniform, &history[i].view, &history[i].view)
            }),
            horizontal: std::array::from_fn(|i| {
                lib.post_group(device, uniform, &history[i].view, &history[i].view)
            }),
            vertical: lib.post_group(device, uniform, &blur[0].view, &blur[0].view),
            composite: std::array::from_fn(|i| {
                lib.post_group(device, uniform, &history[i].view, &blur[1].view)
            }),
            size,
            history,
            blur,
        }
    }
}
/// Per-component state. Dropping the instance releases buffers/history; cloning
/// settings never clones GPU resources. Quality changes rebuild only this instance.
pub struct OrbitalGpu {
    uniform: w::Buffer,
    #[cfg(test)]
    particles: [w::Buffer; 2],
    groups: [w::BindGroup; 2],
    count: u32,
    current: usize,
    image: usize,
    initialized: bool,
    camera: Option<[[f32; 4]; 5]>,
    path_groups: [w::BindGroup; 2],
    render_groups: [w::BindGroup; 2],
    #[cfg(test)]
    path_buffer: w::Buffer,
    path_head: u32,
    path_valid: u32,
    path_elapsed: f32,
    paths_enabled: bool,
    parameters: Option<Frame>,
    images: Images,
}
fn pass<'a>(encoder: &'a mut w::CommandEncoder, target: &w::TextureView) -> w::RenderPass<'a> {
    encoder.begin_render_pass(&w::RenderPassDescriptor {
        label: Some("Visualizer HDR pass"),
        color_attachments: &[Some(w::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: w::Operations {
                load: w::LoadOp::Clear(w::Color::BLACK),
                store: w::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
impl OrbitalGpu {
    pub fn new(device: &w::Device, lib: &Library, count: u32, size: [u32; 2]) -> Self {
        let count = count.clamp(128, 131_072);
        let uniform = device.create_buffer_init(&w::util::BufferInitDescriptor {
            label: Some("Visualizer parameters"),
            contents: bytemuck::bytes_of(&Frame::zeroed()),
            usage: w::BufferUsages::UNIFORM | w::BufferUsages::COPY_DST,
        });
        let particles: [w::Buffer; 2] = std::array::from_fn(|_| {
            device.create_buffer(&w::BufferDescriptor {
                label: Some("GPU particles"),
                size: count as u64 * 32,
                usage: w::BufferUsages::STORAGE
                    | if cfg!(test) {
                        w::BufferUsages::COPY_SRC | w::BufferUsages::COPY_DST
                    } else {
                        w::BufferUsages::empty()
                    },
                mapped_at_creation: false,
            })
        });
        let grid = device.create_buffer(&w::BufferDescriptor {
            label: Some("Flocking spatial grid"),
            size: 4096 * 32,
            usage: w::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let path_buffer = device.create_buffer(&w::BufferDescriptor {
            label: Some("512 tracer paths × 128 samples"),
            size: 512 * 128 * 16,
            usage: w::BufferUsages::STORAGE
                | if cfg!(test) {
                    w::BufferUsages::COPY_SRC
                } else {
                    w::BufferUsages::empty()
                },
            mapped_at_creation: false,
        });
        let render_groups = std::array::from_fn(|i| {
            device.create_bind_group(&w::BindGroupDescriptor {
                label: Some("Particle draw resources"),
                layout: &lib.render_layout,
                entries: &[
                    w::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 1,
                        resource: particles[i].as_entire_binding(),
                    },
                ],
            })
        });
        let path_groups = std::array::from_fn(|i| {
            device.create_bind_group(&w::BindGroupDescriptor {
                label: Some("Tracer rendering"),
                layout: &lib.path_layout,
                entries: &[
                    w::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 1,
                        resource: path_buffer.as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 2,
                        resource: particles[i].as_entire_binding(),
                    },
                ],
            })
        });
        let groups = std::array::from_fn(|i| {
            device.create_bind_group(&w::BindGroupDescriptor {
                label: Some("Orbital ping pong"),
                layout: &lib.layout,
                entries: &[
                    w::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 1,
                        resource: particles[i].as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 2,
                        resource: particles[1 - i].as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 3,
                        resource: grid.as_entire_binding(),
                    },
                    w::BindGroupEntry {
                        binding: 4,
                        resource: path_buffer.as_entire_binding(),
                    },
                ],
            })
        });
        let images = Images::new(device, lib, &uniform, size);
        Self {
            uniform,
            #[cfg(test)]
            particles,
            groups,
            count,
            current: 0,
            image: 0,
            initialized: false,
            camera: None,
            path_groups,
            render_groups,
            #[cfg(test)]
            path_buffer,
            path_head: 127,
            path_valid: 0,
            path_elapsed: 0.0,
            paths_enabled: false,
            parameters: None,
            images,
        }
    }
    pub fn count(&self) -> u32 {
        self.count
    }
    pub fn prepare(
        &mut self,
        device: &w::Device,
        queue: &w::Queue,
        encoder: &mut w::CommandEncoder,
        lib: &Library,
        mut frame: Frame,
        size: [u32; 2],
    ) {
        let camera = [
            frame.view,
            frame.axis_x,
            frame.axis_y,
            frame.axis_z,
            frame.camera,
        ];
        let camera_changed = self.camera != Some(camera);
        self.camera = Some(camera);
        let mut parameters = frame;
        parameters.clock = [0.0; 4];
        parameters.output = [0.0; 4];
        let parameters_changed = self.parameters != Some(parameters);
        self.parameters = Some(parameters);
        let paused_edit = frame.clock[0] == 0.0 && parameters_changed;
        let resized = self.images.size != size;
        if resized {
            self.images = Images::new(device, lib, &self.uniform, size);
        }
        if self.initialized
            && frame.clock[0] == 0.0
            && !resized
            && !camera_changed
            && !parameters_changed
        {
            return;
        }
        frame.clock[2] = self.count as f32;
        frame.clock[3] = if self.initialized { 0.0 } else { 1.0 };
        frame.output = [
            size[0] as f32,
            size[1] as f32,
            if lib.output_srgb { 1.0 } else { 0.0 },
            if camera_changed || resized || paused_edit {
                1.0
            } else {
                0.0
            },
        ];
        let paths_enabled = frame.paths[0] > 0.5;
        if paths_enabled != self.paths_enabled {
            self.path_valid = 0;
            self.path_head = 127;
            self.path_elapsed = 0.0;
            // Clear screen-space feedback when switching paths on/off.
            frame.output[3] = 1.0;
        }
        self.paths_enabled = paths_enabled;
        if paths_enabled {
            self.path_elapsed += frame.clock[0];
        }
        let record = paths_enabled
            && (self.path_valid == 0 || self.path_elapsed >= 1.0 / 30.0)
            && (frame.clock[0] > 0.0 || !self.initialized);
        if record {
            self.path_elapsed = self.path_elapsed.rem_euclid(1.0 / 30.0);
            self.path_head = (self.path_head + 1) % 128;
            self.path_valid = (self.path_valid + 1).min(128);
        }
        let stride = self.count.div_ceil(512);
        frame.paths[1] = frame.paths[1].clamp(1.0, self.count.div_ceil(stride) as f32);
        frame.path_clock = [
            self.path_head as f32,
            self.path_valid as f32,
            if record { 1.0 } else { 0.0 },
            stride as f32,
        ];
        // Resizing clears history naturally through fresh, zero-initialized textures.
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&frame));
        if frame.clock[0] > 0.0 || !self.initialized {
            let mut pass = encoder.begin_compute_pass(&w::ComputePassDescriptor {
                label: Some("Spatial flocking"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &self.groups[self.current], &[]);
            pass.set_pipeline(&lib.clear);
            pass.dispatch_workgroups(32, 1, 1);
            pass.set_pipeline(&lib.bin);
            pass.dispatch_workgroups(self.count.div_ceil(128), 1, 1);
            pass.set_pipeline(&lib.simulate);
            pass.dispatch_workgroups(self.count.div_ceil(128), 1, 1);
            self.current = 1 - self.current;
        }
        let dest = 1 - self.image;
        {
            let mut render = pass(encoder, &self.images.history[dest].view);
            render.set_pipeline(&lib.history);
            render.set_bind_group(0, &self.images.feedback[self.image], &[]);
            render.draw(0..3, 0..1);
            render.set_pipeline(&lib.particles);
            render.set_bind_group(0, &self.render_groups[self.current], &[]);
            render.draw(0..6, 0..self.count + frame.orbs[0] as u32);
            if paths_enabled && self.path_valid > 1 {
                render.set_pipeline(&lib.paths);
                render.set_bind_group(0, &self.path_groups[self.current], &[]);
                render.draw(0..6, 0..frame.paths[1] as u32 * (self.path_valid - 1));
            }
        }
        {
            let mut render = pass(encoder, &self.images.blur[0].view);
            render.set_pipeline(&lib.horizontal);
            render.set_bind_group(0, &self.images.horizontal[dest], &[]);
            render.draw(0..3, 0..1);
        }
        {
            let mut render = pass(encoder, &self.images.blur[1].view);
            render.set_pipeline(&lib.vertical);
            render.set_bind_group(0, &self.images.vertical, &[]);
            render.draw(0..3, 0..1);
        }
        self.image = dest;
        self.initialized = true;
    }
    /// Composite into the host's current viewport/scissor. The host owns clipping.
    pub fn paint(&self, lib: &Library, pass: &mut w::RenderPass<'_>) {
        pass.set_pipeline(&lib.composite);
        pass.set_bind_group(0, &self.images.composite[self.image], &[]);
        pass.draw(0..3, 0..1);
    }
}
use bytemuck::Zeroable;

#[cfg(test)]
impl OrbitalGpu {
    pub fn verify_optical_depth(
        device: &w::Device,
        queue: &w::Queue,
        lib: &Library,
        mut frame: Frame,
    ) {
        let mut gpu = Self::new(device, lib, 128, [64, 64]);
        frame.clock = [0.0, 0.0, 128.0, 0.0];
        frame.orbs = [1.0, 0.0, 0.6, 0.0];
        frame.axis_x = [1.0, 0.0, 0.0, 0.0];
        frame.axis_y = [0.0, 1.0, 0.0, 0.0];
        frame.axis_z = [0.0, 0.0, 1.0, 0.0];
        frame.view = [1.0, 1.0, 0.0, 0.0];
        frame.orb_style = [7.0, 1.0, 1.0, 1.0];
        frame.orb_shape = [0.0; 4];
        frame.orb_optics = [1.0, 0.0, 2.8, 0.0];
        frame.orb_corona = [0.0; 4];
        frame.orb_flare = [48.0, 0.0, 0.0, 0.0];
        frame.particle_light = [1.0, 0.0, 0.0, 0.0]; // Hidden light, but a visible/occluding surface.
        frame.particle_style = [1.0, 0.0, 1.0, 0.0];
        frame.look = [4.0, 0.0, 1.0, 0.0];
        frame.light = [0.0, 0.0, 1.0, 0.0];
        let target = device.create_texture(&w::TextureDescriptor {
            label: Some("orb depth proof"),
            size: w::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::RENDER_ATTACHMENT | w::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let read = device.create_buffer(&w::BufferDescriptor {
            label: Some("orb depth pixels"),
            size: 64 * 64 * 4,
            usage: w::BufferUsages::COPY_DST | w::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut draw = |z: f32, opacity: f32, orthographic: f32| {
            let particle: [f32; 8] = [0.0, 0.0, z, 0.5, 1.0, 0.0, 0.0, 0.0];
            queue.write_buffer(
                &gpu.particles[gpu.current],
                0,
                bytemuck::cast_slice(&vec![particle; 128]),
            );
            gpu.initialized = true;
            // Force redraw after changing fixture data while paused.
            gpu.parameters = None;
            frame.orb_optics[0] = opacity;
            frame.camera[2] = orthographic;
            let mut encoder = device.create_command_encoder(&Default::default());
            gpu.prepare(device, queue, &mut encoder, lib, frame, [64, 64]);
            {
                let mut render = pass(&mut encoder, &view);
                gpu.paint(lib, &mut render);
            }
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                w::TexelCopyBufferInfo {
                    buffer: &read,
                    layout: w::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(64),
                    },
                },
                target.size(),
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            read.slice(..)
                .map_async(w::MapMode::Read, move |r| tx.send(r).unwrap());
            device.poll(w::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let bytes = read.slice(..).get_mapped_range().unwrap();
            let light: usize = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| usize::from(p[0]) + usize::from(p[1]) + usize::from(p[2]))
                .sum();
            drop(bytes);
            read.unmap();
            light
        };
        for ortho in [0.0, 1.0] {
            assert_eq!(
                draw(1.0, 1.0, ortho),
                0,
                "particles behind the core must be hidden"
            );
            assert!(
                draw(-1.0, 1.0, ortho) > 100,
                "particles in front must remain visible"
            );
            assert!(
                draw(1.0, 0.0, ortho) > 100,
                "transparent cores keep particles behind visible"
            );
        }
        eprintln!(
            "Optical depth: front particles remain visible; rear particles occlude in perspective and orthographic views"
        );
    }
    pub fn verify_surface_fields(
        device: &w::Device,
        queue: &w::Queue,
        lib: &Library,
        mut frame: Frame,
    ) {
        let mut gpu = Self::new(device, lib, 128, [64, 64]);
        frame.clock = [1.0 / 15.0, 0.0, 128.0, 0.0];
        frame.orbs = [1.0, 0.0, 0.1, 0.0];
        frame.orb_style[1] = 1.0;
        frame.orb_style[2] = 1.0;
        frame.orb_shape = [0.0; 4];
        frame.flock = [0.0, 0.0, 0.0, 6.0];
        frame.forces = [0.0; 4];
        frame.dynamics = [0.0; 4];
        frame.field[0] = 0.0;
        frame.field[3] = 0.0;
        frame.orb_field = [1.0, 0.0, 0.8, 0.0];
        frame.orb_barrier = [0.0, 1.5, 1.0, 0.0];
        let seed: [f32; 8] = [0.2, 0.0, 0.0, 0.5, -6.0, 0.0, 0.0, 0.0];
        let mut step = |f: Frame, reset: bool| {
            if reset {
                queue.write_buffer(
                    &gpu.particles[gpu.current],
                    0,
                    bytemuck::cast_slice(&vec![seed; 128]),
                );
                gpu.initialized = true;
            }
            let mut encoder = device.create_command_encoder(&Default::default());
            gpu.prepare(device, queue, &mut encoder, lib, f, [64, 64]);
            queue.submit([encoder.finish()]);
            gpu.read_particles(device, queue)
        };
        // Both endpoints are outside, but the segment crosses the entire orb.
        let solid = step(frame, true);
        assert!(
            solid.iter().all(|p| p[0] >= 0.1004),
            "sweep must prevent tunneling: {:?}",
            solid[0]
        );
        frame.orb_barrier[2] = 0.0;
        assert!(
            step(frame, true).iter().all(|p| p[0] < -0.19),
            "soft field permits passage at zero force"
        );
        frame.orb_field[3] = 30.0;
        frame.orb_barrier[0] = 8.0;
        let deflected = step(frame, true);
        assert!(
            deflected.iter().all(|p| p[0] > -0.15 && p[2].abs() > 0.01),
            "soft field must repel and circulate without solid collision"
        );
        frame.orb_barrier[2] = 1.0;
        frame.orb_field[0] = 0.0;
        assert!(
            step(frame, true).iter().all(|p| p[0] < -0.19),
            "disabled field preserves passage"
        );
        frame.orb_field = [1.0, 0.2, 0.8, 12.0];
        frame.orb_barrier = [2.0, 1.5, 1.0, 0.0];
        frame.orbs = [3.0, 2.0, 0.3, 0.8];
        frame.orb_style[1] = 1.5;
        frame.orb_shape = [0.0, 0.4, 0.7, 0.5];
        frame.audio[0] = 0.6;
        frame.forces = [5.0, 4.0, 0.0, 3.0];
        for tick in 0..120 {
            frame.clock[1] += frame.clock[0];
            let particles = step(frame, tick == 0);
            for j in 0..3 {
                let angle = j as f32 * std::f32::consts::TAU / 3.0 + frame.clock[1] * 0.8;
                let center = [
                    angle.cos() * 2.0,
                    (angle * 1.3).sin() * 0.7,
                    angle.sin() * 2.0,
                ];
                let radius = 0.3
                    * 1.5
                    * (1.0
                        + 0.4
                            * (frame.clock[1] * 0.7 * std::f32::consts::TAU + j as f32 * 1.7)
                                .sin())
                    * 1.3
                    + 0.2;
                for p in &particles {
                    assert!(p.iter().all(|v| v.is_finite()));
                    let distance = ((p[0] - center[0]).powi(2)
                        + (p[1] - center[1]).powi(2)
                        + (p[2] - center[2]).powi(2))
                    .sqrt();
                    assert!(
                        distance >= radius - 0.0001,
                        "moving / pulsing surface penetration: {distance} < {radius}"
                    );
                }
            }
        }
        eprintln!(
            "Orb fields: swept collision, disabled/soft passage, moving and pulsing surface clearance verified"
        );
    }
    pub fn verify_paths(&self, device: &w::Device, queue: &w::Queue) {
        let read = device.create_buffer(&w::BufferDescriptor {
            label: Some("path verification"),
            size: 512 * 128 * 16,
            usage: w::BufferUsages::COPY_DST | w::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.path_buffer, 0, &read, 0, 512 * 128 * 16);
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..)
            .map_async(w::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(w::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let bytes = read.slice(..).get_mapped_range().unwrap();
        let points: &[[f32; 4]] = bytemuck::cast_slice(&bytes);

        assert_eq!(self.path_valid, 128);
        assert!(points.iter().all(|p| p.iter().all(|v| v.is_finite())));
        assert!(
            points[self.path_head as usize * 512][3]
                > points[((self.path_head + 127) % 128) as usize * 512][3]
        );
    }
    fn read_particles(&self, device: &w::Device, queue: &w::Queue) -> Vec<[f32; 8]> {
        let size = self.count as u64 * 32;
        let read = device.create_buffer(&w::BufferDescriptor {
            label: Some("particle verification"),
            size,
            usage: w::BufferUsages::MAP_READ | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.particles[self.current], 0, &read, 0, size);
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..)
            .map_async(w::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(w::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let bytes = read.slice(..).get_mapped_range().unwrap();
        bytemuck::cast_slice(&bytes).to_vec()
    }
    pub fn verify_cloud(&self, device: &w::Device, queue: &w::Queue) -> usize {
        let particles = self.read_particles(device, queue);
        let mut cells = std::collections::HashSet::new();
        for p in particles {
            assert!(p.iter().all(|v| v.is_finite()), "finite simulation state");
            assert!(p[..3].iter().all(|v| v.abs() < 12.0), "bounded cloud");
            cells.insert((
                (p[0] * 4.0).floor() as i32,
                (p[1] * 4.0).floor() as i32,
                (p[2] * 4.0).floor() as i32,
            ));
        }

        eprintln!("Cloud occupies {} quarter-unit cells", cells.len());
        cells.len()
    }
}
