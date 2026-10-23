//! Spectrum-attached GPU particles with spring return, layered transforms and optical sprites.
use wgpu::{self as w, util::DeviceExt};
pub const BANDS: usize = 96;
pub const ROWS: usize = 64;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Layer {
    pub transform: [f32; 4],
    pub rotation: [f32; 4],
    pub style: [f32; 4],
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub c: [f32; 4],
    pub gradient: [[f32; 4]; 8],
    pub palette: [f32; 4], // count (zero keeps legacy blend), cyclic, reserved
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Frame {
    pub clock: [f32; 4],
    pub geometry: [f32; 4],
    pub layout: [f32; 4],
    pub motion: [f32; 4],
    pub response: [f32; 4],
    pub ripple: [f32; 4],
    pub look: [f32; 4],
    pub optics: [f32; 4],
    pub color: [f32; 4],
    pub light: [f32; 4],
    pub view: [f32; 4],
    pub camera: [f32; 4],
    pub axis_x: [f32; 4],
    pub axis_y: [f32; 4],
    pub axis_z: [f32; 4],
    pub output: [f32; 4],
    pub sculpture: [f32; 4],
    pub fragments: [f32; 4],
    pub feedback: [f32; 4],
    pub feedback_shift: [f32; 4],
    pub layers: [Layer; 8],
}
pub struct Library {
    sim: w::ComputePipeline,
    grains: w::RenderPipeline,
    composite: w::RenderPipeline,
    feedback: w::RenderPipeline,
    feedback_layout: w::BindGroupLayout,
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
impl Library {
    pub fn new(d: &w::Device, format: w::TextureFormat) -> Self {
        let sim_layout = d.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Particle Waves compute"),
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
            label: Some("Particle Waves render"),
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
            label: Some("Particle Waves composite"),
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
            label: Some("Particle Waves"),
            source: w::ShaderSource::Wgsl(include_str!("particle_waves.wgsl").into()),
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
            label: Some("Particle Waves grain step"),
            layout: Some(&sl),
            module: &shader,
            entry_point: Some("simulate"),
            compilation_options: Default::default(),
            cache: None,
        });
        let source = include_str!("particle_waves.wgsl");
        let start = source.find("@compute").unwrap();
        let end = source.find("// RENDER_START").unwrap();
        let render_source = format!("{}{}", &source[..start], &source[end..])
            .replace("var<storage,read_write> grains", "var<storage,read> grains");
        let render_shader = d.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Particle Waves drawing"),
            source: w::ShaderSource::Wgsl(render_source.into()),
        });
        let make = |vertex, fragment| {
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
                        format: w::TextureFormat::Rgba16Float,
                        blend: Some(w::BlendState {
                            color: w::BlendComponent {
                                src_factor: w::BlendFactor::One,
                                dst_factor: w::BlendFactor::One,
                                operation: w::BlendOperation::Add,
                            },
                            alpha: w::BlendComponent {
                                src_factor: w::BlendFactor::One,
                                dst_factor: w::BlendFactor::OneMinusSrcAlpha,
                                operation: w::BlendOperation::Add,
                            },
                        }),
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
        };
        let grains = make("vertex", "fragment");
        let feedback_layout = d.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label: Some("Spectral Bloom feedback parameters"),
            entries: &[entry(
                0,
                w::BufferBindingType::Uniform,
                w::ShaderStages::FRAGMENT,
            )],
        });
        let blit = d.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Particle Waves blit"),
            source: w::ShaderSource::Wgsl(include_str!("particle_waves_blit.wgsl").into()),
        });
        let pl = d.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&texture_layout), Some(&feedback_layout)],
            immediate_size: 0,
        });
        let composite = d.create_render_pipeline(&w::RenderPipelineDescriptor {
            label: Some("Particle Waves composite"),
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
        let feedback_shader = d.create_shader_module(w::ShaderModuleDescriptor {
            label: Some("Spectral Bloom feedback"),
            source: w::ShaderSource::Wgsl(include_str!("spectral_bloom_feedback.wgsl").into()),
        });
        let fl = d.create_pipeline_layout(&w::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&texture_layout), Some(&feedback_layout)],
            immediate_size: 0,
        });
        let feedback = d.create_render_pipeline(&w::RenderPipelineDescriptor {
            label: Some("Spectral Bloom image feedback"),
            layout: Some(&fl),
            vertex: w::VertexState {
                module: &feedback_shader,
                entry_point: Some("vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(w::FragmentState {
                module: &feedback_shader,
                entry_point: Some("fragment"),
                targets: &[Some(w::ColorTargetState {
                    format: w::TextureFormat::Rgba16Float,
                    blend: None,
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
            grains,
            composite,
            feedback,
            feedback_layout,
            sim_layout,
            draw_layout,
            texture_layout,
            sampler,
            format,
        }
    }
}
pub struct Gpu {
    uniform: w::Buffer,
    field: w::Buffer,
    pub particles: w::Buffer,
    sim: w::BindGroup,
    draw: w::BindGroup,
    texture: w::Texture,
    view: w::TextureView,
    blit: w::BindGroup,
    previous: Option<(w::Texture, w::TextureView, w::BindGroup)>,
    feedback_uniform: w::Buffer,
    feedback_bind: w::BindGroup,
    feedback_valid: bool,
    last_frame: Option<Frame>,
    size: [u32; 2],
    pub count: u32,
    first: bool,
}
fn target(
    d: &w::Device,
    l: &Library,
    size: [u32; 2],
) -> (w::Texture, w::TextureView, w::BindGroup) {
    let descriptor = w::TextureDescriptor {
        label: Some("Particle Waves target"),
        size: w::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: w::TextureDimension::D2,
        format: w::TextureFormat::Rgba16Float,
        usage: w::TextureUsages::RENDER_ATTACHMENT
            | w::TextureUsages::TEXTURE_BINDING
            | w::TextureUsages::COPY_SRC,
        view_formats: &[],
    };
    let texture = d.create_texture(&descriptor);
    let view = texture.create_view(&Default::default());
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
    (texture, view, blit)
}
impl Gpu {
    pub fn new(d: &w::Device, l: &Library, count: u32, size: [u32; 2]) -> Self {
        let uniform = d.create_buffer_init(&w::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&Frame::zero()),
            usage: w::BufferUsages::UNIFORM | w::BufferUsages::COPY_DST,
        });
        let field = d.create_buffer(&w::BufferDescriptor {
            label: Some("Particle Waves spectral history"),
            size: (BANDS * ROWS * 4) as u64,
            usage: w::BufferUsages::STORAGE | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let particles = d.create_buffer(&w::BufferDescriptor {
            label: Some("Particle Waves grains"),
            size: count as u64 * 32,
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
        let (texture, view, blit) = target(d, l, size);
        let feedback_uniform = d.create_buffer(&w::BufferDescriptor {
            label: Some("Feedback parameters"),
            size: 64,
            usage: w::BufferUsages::UNIFORM | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let feedback_bind = d.create_bind_group(&w::BindGroupDescriptor {
            label: None,
            layout: &l.feedback_layout,
            entries: &[w::BindGroupEntry {
                binding: 0,
                resource: feedback_uniform.as_entire_binding(),
            }],
        });
        Self {
            uniform,
            field,
            particles,
            sim,
            draw,
            texture,
            view,
            blit,
            previous: None,
            feedback_uniform,
            feedback_bind,
            feedback_valid: false,
            last_frame: None,
            size,
            count,
            first: true,
        }
    }
    pub fn prepare(
        &mut self,
        d: &w::Device,
        e: &mut w::CommandEncoder,
        l: &Library,
        mut frame: Frame,
        field: &[f32],
        size: [u32; 2],
    ) {
        if size != self.size {
            (self.texture, self.view, self.blit) = target(d, l, size);
            self.size = size;
            self.previous = None;
            self.feedback_valid = false;
            self.last_frame = None;
        }
        frame.output = [
            size[0] as f32,
            size[1] as f32,
            if l.format.is_srgb() { 1.0 } else { 0.0 },
            self.count as f32,
        ];
        let feedback_enabled = frame.feedback[0] > 0.0;
        let mut comparison = frame;
        comparison.clock[0] = 0.0;
        comparison.clock[2] = 0.0;
        if feedback_enabled && frame.clock[0] == 0.0 && self.last_frame == Some(comparison) {
            return;
        }
        if frame.clock[0] == 0.0 {
            self.feedback_valid = false;
        }
        self.last_frame = Some(comparison);
        if feedback_enabled {
            let previous = self.previous.get_or_insert_with(|| target(d, l, size));
            std::mem::swap(&mut self.texture, &mut previous.0);
            std::mem::swap(&mut self.view, &mut previous.1);
            std::mem::swap(&mut self.blit, &mut previous.2);
        } else {
            self.previous = None;
            self.feedback_valid = false;
        }
        if self.first {
            frame.clock[2] = 1.0;
            self.first = false;
        }
        frame.feedback_shift[3] = if self.feedback_valid { 0.0 } else { 1.0 };
        // Queue::write_buffer runs before all command buffers in the next
        // submission. A later callback could therefore overwrite an earlier
        // callback's reset flag. Encode uploads before their own dispatch so
        // batched preparations each see the intended uniform and spectral history.
        let uniform = bytemuck::bytes_of(&frame);
        let feedback_values: [[f32; 4]; 4] = [
            frame.feedback,
            frame.feedback_shift,
            [
                frame.clock[0].clamp(0.0, 0.1),
                frame.clock[1],
                frame.response[0],
                0.0,
            ],
            [size[0] as f32, size[1] as f32, frame.sculpture[0], 0.0],
        ];
        let feedback_bytes = bytemuck::bytes_of(&feedback_values);
        let field = bytemuck::cast_slice(field);
        let mut bytes = Vec::with_capacity(uniform.len() + field.len());
        bytes.extend_from_slice(uniform);
        bytes.extend_from_slice(field);
        bytes.extend_from_slice(feedback_bytes);
        let upload = d.create_buffer_init(&w::util::BufferInitDescriptor {
            label: Some("Particle Waves ordered frame upload"),
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
        e.copy_buffer_to_buffer(
            &upload,
            (uniform.len() + field.len()) as u64,
            &self.feedback_uniform,
            0,
            64,
        );
        {
            let mut pass = e.begin_compute_pass(&w::ComputePassDescriptor::default());
            pass.set_pipeline(&l.sim);
            pass.set_bind_group(0, &self.sim, &[]);
            pass.dispatch_workgroups(self.count.div_ceil(128), 1, 1);
        }
        {
            let mut pass = e.begin_render_pass(&w::RenderPassDescriptor {
                label: Some("Particle Waves offscreen"),
                color_attachments: &[Some(w::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: w::Operations {
                        load: w::LoadOp::Clear(w::Color::TRANSPARENT),
                        store: w::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if feedback_enabled && self.feedback_valid {
                pass.set_pipeline(&l.feedback);
                pass.set_bind_group(0, &self.previous.as_ref().unwrap().2, &[]);
                pass.set_bind_group(1, &self.feedback_bind, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_pipeline(&l.grains);
            pass.set_bind_group(0, &self.draw, &[]);
            pass.set_pipeline(&l.grains);
            pass.draw(0..6, 0..self.count);
        }
        self.feedback_valid = feedback_enabled;
    }
    pub fn paint(&self, l: &Library, pass: &mut w::RenderPass<'_>) {
        pass.set_pipeline(&l.composite);
        pass.set_bind_group(0, &self.blit, &[]);
        pass.set_bind_group(1, &self.feedback_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
impl Frame {
    pub fn zero() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame {
        Frame {
            clock: [1.0 / 60.0, 0.0, 0.0, 0.25],
            geometry: [5.5, 5.0, 1.6, 4.0],
            layout: [0.6, 1.5, 2.0, 0.0],
            motion: [0.3, 9.0, 4.0, 1.3],
            response: [0.8, 0.18, 1.5, 1.0],
            ripple: [0.15, 3.0, 1.0, 0.15],
            look: [1.6, 0.7, 0.85, 0.85],
            optics: [8.0, 0.7, 24.0, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
            light: [0.45, 1.1, 0.4, 0.6],
            view: [16.0 / 9.0, 1.0, 0.0, 0.0],
            camera: [8.5, 53.13_f32.to_radians(), 0.0, 0.35],
            axis_x: [0.852, -0.135, 0.505, 0.0],
            axis_y: [-0.106, -0.982, -0.156, 0.0],
            axis_z: [-0.512, -0.08, 0.855, 0.0],
            output: [0.0; 4],
            sculpture: [0.0; 4],
            fragments: [0.0; 4],
            feedback: [0.0; 4],
            feedback_shift: [0.0; 4],
            layers: [Layer {
                transform: [0.0, 0.0, 0.0, 1.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                style: [1.0; 4],
                a: [0.24, 0.47, 1.0, 1.0],
                b: [0.53, 1.0, 0.13, 1.0],
                c: [0.92, 1.0, 1.0, 1.0],
                gradient: [[0.0; 4]; 8],
                palette: [0.0; 4],
            }; 8],
        }
    }
    fn read(d: &w::Device, q: &w::Queue, b: &w::Buffer, size: u64) -> Vec<u8> {
        let out = d.create_buffer(&w::BufferDescriptor {
            label: None,
            size,
            usage: w::BufferUsages::COPY_DST | w::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = d.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(b, 0, &out, 0, size);
        q.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        out.slice(..)
            .map_async(w::MapMode::Read, move |r| tx.send(r).unwrap());
        d.poll(w::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        out.slice(..).get_mapped_range().unwrap().to_vec()
    }
    fn offsets(d: &w::Device, q: &w::Queue, g: &Gpu) -> Vec<f32> {
        read(d, q, &g.particles, g.count as u64 * 32)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_ne_bytes(*v))
            .collect()
    }
    fn distance(values: &[f32]) -> f32 {
        values
            .as_chunks::<8>()
            .0
            .iter()
            .map(|p| p[..3].iter().map(|v| v * v).sum::<f32>())
            .sum::<f32>()
            / (values.len() / 8) as f32
    }
    fn step(
        d: &w::Device,
        q: &w::Queue,
        l: &Library,
        g: &mut Gpu,
        f: Frame,
        field: &[f32],
        size: [u32; 2],
    ) {
        let mut e = d.create_command_encoder(&Default::default());
        g.prepare(d, &mut e, l, f, field, size);
        q.submit([e.finish()]);
    }
    fn pixels(d: &w::Device, q: &w::Queue, g: &Gpu) -> Vec<u8> {
        let l = Library::new(d, w::TextureFormat::Rgba8Unorm);
        let out = d.create_texture(&w::TextureDescriptor {
            label: None,
            size: w::Extent3d {
                width: g.size[0],
                height: g.size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::RENDER_ATTACHMENT | w::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = out.create_view(&Default::default());
        let stride = (g.size[0] * 4).div_ceil(256) * 256;
        let buffer = d.create_buffer(&w::BufferDescriptor {
            label: None,
            size: (stride * g.size[1]) as u64,
            usage: w::BufferUsages::COPY_DST | w::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mut e = d.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_render_pass(&w::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(w::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: w::Operations {
                        load: w::LoadOp::Clear(w::Color::TRANSPARENT),
                        store: w::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            g.paint(&l, &mut pass);
        }
        e.copy_texture_to_buffer(
            out.as_image_copy(),
            w::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: w::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            w::Extent3d {
                width: g.size[0],
                height: g.size[1],
                depth_or_array_layers: 1,
            },
        );
        q.submit([e.finish()]);
        read(d, q, &buffer, (stride * g.size[1]) as u64)
            .chunks(stride as usize)
            .flat_map(|row| row[..g.size[0] as usize * 4].iter().copied())
            .collect()
    }
    fn light(pixels: &[u8]) -> u64 {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[..3].iter().map(|v| *v as u64).sum::<u64>())
            .sum()
    }
    #[test]
    #[ignore = "requires a real compute GPU"]
    fn gpu_spectral_bloom_feedback_decay_pause_resize_and_render() {
        pollster::block_on(async {
            let instance = w::Instance::default();
            let adapter = instance
                .request_adapter(&w::RequestAdapterOptions::default())
                .await
                .unwrap();
            let (d, q) = adapter
                .request_device(&w::DeviceDescriptor::default())
                .await
                .unwrap();
            let scope = d.push_error_scope(w::ErrorFilter::Validation);
            let l = Library::new(&d, w::TextureFormat::Rgba8Unorm);
            let size = [1280, 720];
            let mut g = Gpu::new(&d, &l, 32768, size);
            let mut f = frame();
            f.geometry = [5.0, 2.8, 4.5, 3.0];
            f.layout = [0.2, 2.3, 1.6, 0.0];
            f.ripple[0] = 0.0;
            f.sculpture = [1.0, 96.0, 1.1, 0.45];
            f.fragments = [0.22, 0.35, 0.65, 1.0];
            f.feedback = [0.35, 0.06, 0.025, 0.1];
            f.feedback_shift = [0.0, 0.025, 0.5, 0.0];
            f.look = [1.15, 0.4, 0.7, 1.0];
            f.optics[1] = 0.12;
            f.light = [0.35, 1.1, 0.4, 0.6];
            f.color[0] = 1.0;
            f.view[1] = 1.1;
            for (i, layer) in f.layers.iter_mut().enumerate() {
                layer.transform = [
                    i as f32 * 0.2,
                    -(i as f32) * 0.55,
                    i as f32 * 0.2,
                    (1.0 - i as f32 * 0.1).max(0.4),
                ];
                layer.rotation[2] = 0.9;
                layer.a = [0.28, 0.69, 1.0, 1.0];
                layer.b = [1.0, 0.49, 0.61, 1.0];
                layer.c = [0.96, 0.95, 0.84, 1.0];
                layer.gradient = [
                    layer.a, layer.b, layer.c, layer.a, layer.b, layer.c, layer.a, layer.b,
                ];
                layer.palette = [8.0, 0.0, 0.0, 0.0];
            }
            let field: Vec<f32> = (0..ROWS)
                .flat_map(|y| {
                    (0..BANDS).map(move |x| {
                        let u = x as f32 / BANDS as f32;
                        let t = y as f32 / ROWS as f32;
                        (0.7 * (-((u - 0.22) / 0.17).powi(2)).exp()
                            + 0.95 * (-((u - 0.72) / 0.14).powi(2)).exp())
                            * (0.85 + 0.15 * (t * 10.0 + u * 6.0).cos())
                    })
                })
                .collect();
            let mut timings = Vec::new();
            for i in 0..120 {
                f.clock[1] = i as f32 / 120.0;
                f.clock[0] = 1.0 / 120.0;
                let start = std::time::Instant::now();
                step(&d, &q, &l, &mut g, f, &field, size);
                d.poll(w::PollType::wait_indefinitely()).unwrap();
                if i > 5 {
                    timings.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            timings.sort_by(f64::total_cmp);
            eprintln!(
                "Bloom 32,768 grains + feedback, 720p: median {:.3} ms, p95 {:.3} ms",
                timings[timings.len() / 2],
                timings[timings.len() * 95 / 100]
            );
            let initial = pixels(&d, &q, &g);
            assert!(light(&initial) > 100000, "sculpture must be visible");
            let mut ppm = b"P6\n1280 720\n255\n".to_vec();
            for pixel in initial.as_chunks::<4>().0.iter() {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(std::env::temp_dir().join("symphos-spectral-bloom.ppm"), ppm).unwrap();
            f.clock[0] = 0.0;
            step(&d, &q, &l, &mut g, f, &field, size);
            assert_eq!(
                initial,
                pixels(&d, &q, &g),
                "pause must freeze feedback without accumulating light"
            );
            f.clock[0] = 1.0 / 120.0;
            f.light[1] = 0.0;
            step(&d, &q, &l, &mut g, f, &field, size);
            let echo = light(&pixels(&d, &q, &g));
            assert!(
                echo > 1000,
                "previous frame must persist when new emission stops"
            );
            for _ in 0..240 {
                step(&d, &q, &l, &mut g, f, &field, size);
            }
            assert!(
                light(&pixels(&d, &q, &g)) < echo / 20,
                "feedback must decay rather than grow indefinitely"
            );
            // Reset history when resized, never stretch stale pixels into the new frame.
            step(&d, &q, &l, &mut g, f, &field, [322, 182]);
            assert_eq!(light(&pixels(&d, &q, &g)), 0);
            f.light[1] = 0.7;
            step(&d, &q, &l, &mut g, f, &field, size);
            f.feedback[0] = 0.0;
            f.light[1] = 0.0;
            step(&d, &q, &l, &mut g, f, &field, size);
            assert_eq!(
                light(&pixels(&d, &q, &g)),
                0,
                "disabling feedback clears old light"
            );
            let mut decayed = Vec::new();
            for fps in [60, 120] {
                let mut probe = Gpu::new(&d, &l, 8192, [320, 180]);
                let mut fixture = f;
                fixture.feedback = [0.35, 0.0, 0.0, 0.0];
                fixture.feedback_shift = [0.0; 4];
                fixture.light[1] = 0.7;
                fixture.clock[0] = 1.0 / fps as f32;
                step(&d, &q, &l, &mut probe, fixture, &field, [320, 180]);
                fixture.light[1] = 0.0;
                for _ in 0..fps / 2 {
                    step(&d, &q, &l, &mut probe, fixture, &field, [320, 180]);
                }
                decayed.push(light(&pixels(&d, &q, &probe)) as f64);
            }
            assert!(
                (decayed[0] - decayed[1]).abs() / decayed[0].max(1.0) < 0.04,
                "feedback lifetime should agree at 60 and 120 FPS: {decayed:?}"
            );
            assert!(scope.pop().await.is_none());
        });
    }
    #[test]
    fn shader_abi_and_history_size_are_aligned() {
        assert_eq!(std::mem::size_of::<Layer>(), 240);
        assert_eq!(std::mem::offset_of!(Layer, gradient), 96);
        assert_eq!(std::mem::offset_of!(Layer, palette), 224);
        assert_eq!(std::mem::offset_of!(Frame, layers), 320);
        assert_eq!(std::mem::size_of::<Frame>(), 2240);
        assert_eq!(BANDS * ROWS, 6144);
    }
    #[test]
    #[ignore = "requires a real compute GPU"]
    fn gpu_gradient_stops_cyclic_and_layer_overrides_render_without_resetting_particles() {
        pollster::block_on(async {
            let instance = w::Instance::default();
            let adapter = instance.request_adapter(&Default::default()).await.unwrap();
            let (d, q) = adapter.request_device(&Default::default()).await.unwrap();
            let scope = d.push_error_scope(w::ErrorFilter::Validation);
            let l = Library::new(&d, w::TextureFormat::Rgba8Unorm);
            let size = [256, 192];
            let mut g = Gpu::new(&d, &l, 4096, size);
            let mut f = frame();
            f.clock[0] = 0.0;
            f.geometry[3] = 2.0;
            f.optics[1] = 0.0;
            f.look = [4.0, 0.0, 0.5, 1.0];
            f.light = [0.0, 0.3, 0.0, 0.0];
            let field = vec![0.6; BANDS * ROWS];
            let red = [1.0, 0.0, 0.0, 1.0];
            let green = [0.0, 1.0, 0.0, 1.0];
            let blue = [0.0, 0.0, 1.0, 1.0];
            let mut initial = None;
            for mode in 0..5 {
                f.layout[3] = (mode % 4) as f32;
                f.sculpture = if mode == 4 {
                    [1.0, 32.0, 1.0, 0.4]
                } else {
                    [0.0; 4]
                };
                f.fragments = [0.25, 0.3, 0.7, 1.0];
                f.feedback[0] = if mode == 4 { 0.3 } else { 0.0 };
                for (count, cyclic, t, expected) in [
                    (3, false, 0.5, green),
                    (3, false, 0.25, [0.5, 0.5, 0.0, 1.0]),
                    (3, true, 1.0 / 3.0, green),
                    (3, true, 1.0, red),
                    (8, false, 6.0 / 7.0, blue),
                    (8, true, 6.0 / 8.0, blue),
                ] {
                    f.color = [0.0, 0.0, t, 1.0];
                    for layer in &mut f.layers {
                        layer.rotation[2] = 1.0;
                        layer.gradient = [red; 8];
                        layer.gradient[1] = green;
                        layer.gradient[6] = blue;
                        layer.palette = [count as f32, f32::from(cyclic), 0.0, 0.0];
                    }
                    // Each layer carries its own gradient; showing either one
                    // proves it is not accidentally sampling the first layer.
                    f.layers[1].gradient = [blue; 8];
                    for selected in 0..2 {
                        f.layers[0].rotation[3] = f32::from(selected == 0);
                        f.layers[1].rotation[3] = f32::from(selected == 1);
                        step(&d, &q, &l, &mut g, f, &field, size);
                        let pixels = pixels(&d, &q, &g);
                        let sums: [u64; 3] = std::array::from_fn(|c| {
                            pixels.as_chunks::<4>().0.iter().map(|p| p[c] as u64).sum()
                        });
                        let expected = if selected == 0 { expected } else { blue };
                        let total = sums.iter().sum::<u64>() as f64;
                        assert!(total > 1000.0, "mode {mode}, layer {selected} is visible");
                        let expected_total = expected[..3].iter().sum::<f32>() as f64;
                        for c in 0..3 {
                            assert!(
                                (sums[c] as f64 / total - expected[c] as f64 / expected_total)
                                    .abs()
                                    < 0.025,
                                "mode {mode}, layer {selected}, stops {count}, cyclic {cyclic}: {sums:?}, expected {expected:?}"
                            );
                        }
                        let state = offsets(&d, &q, &g);
                        if let Some(initial) = &initial {
                            assert_eq!(&state, initial, "palette edits preserve paused particles");
                        } else {
                            initial = Some(state);
                        }
                    }
                }
            }
            assert!(scope.pop().await.is_none());
        });
    }
    #[test]
    #[ignore = "requires a real compute GPU"]
    fn gpu_particle_waves_drift_return_pause_modes_and_timing() {
        pollster::block_on(async {
            let instance = w::Instance::default();
            let adapter = instance
                .request_adapter(&w::RequestAdapterOptions::default())
                .await
                .unwrap();
            eprintln!("Adapter: {:?}", adapter.get_info());
            let (d, q) = adapter
                .request_device(&w::DeviceDescriptor::default())
                .await
                .unwrap();
            let scope = d.push_error_scope(w::ErrorFilter::Validation);
            let library = Library::new(&d, w::TextureFormat::Rgba8Unorm);
            let size = [1280, 720];
            let field = vec![0.75; BANDS * ROWS];
            let mut end = Vec::new();
            for fps in [60, 120] {
                let mut g = Gpu::new(&d, &library, 8192, size);
                let mut f = frame();
                f.clock[0] = 1.0 / fps as f32;
                for i in 1..=fps {
                    f.clock[1] = i as f32 / fps as f32;
                    step(&d, &q, &library, &mut g, f, &field, size);
                }
                let values = offsets(&d, &q, &g);
                assert!(values.iter().all(|v| v.is_finite()));
                assert!(distance(&values) > 0.002, "audio should detach grains");
                end.push(values.clone());
                f.clock[0] = 0.0;
                step(&d, &q, &library, &mut g, f, &field, [640, 480]);
                assert_eq!(
                    values,
                    offsets(&d, &q, &g),
                    "pause and resize must preserve particle state"
                );
                f.response[0] = 0.0;
                f.clock[0] = 1.0 / fps as f32;
                for i in 1..=fps * 4 {
                    f.clock[1] = 1.0 + i as f32 / fps as f32;
                    step(&d, &q, &library, &mut g, f, &field, [640, 480]);
                }
                assert!(
                    distance(&offsets(&d, &q, &g)) < distance(&values) * 0.001,
                    "silence should return grains to the surface"
                );
            }
            let error = end[0]
                .iter()
                .zip(&end[1])
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            assert!(error < 0.002, "60/120 integration differs: {error}");
            let field: Vec<f32> = (0..ROWS)
                .flat_map(|y| {
                    (0..BANDS).map(move |x| {
                        let u = x as f32 / BANDS as f32;
                        let t = y as f32 / ROWS as f32;
                        ((u * 18.0 - t * 4.0).sin() * 0.25 + 0.4) * (1.0 - u * 0.6)
                    })
                })
                .collect();
            let mut g = Gpu::new(&d, &library, 32768, size);
            let mut f = frame();
            for layer in &mut f.layers {
                layer.gradient = [
                    layer.a, layer.b, layer.c, layer.a, layer.b, layer.c, layer.a, layer.b,
                ];
                layer.palette = [8.0, 0.0, 0.0, 0.0];
            }
            let mut timings = Vec::new();
            for mode in 0..4 {
                f.layout[3] = mode as f32;
                for i in 0..12 {
                    f.clock[1] += 1.0 / 120.0;
                    f.clock[0] = 1.0 / 120.0;
                    let start = std::time::Instant::now();
                    step(&d, &q, &library, &mut g, f, &field, size);
                    d.poll(w::PollType::wait_indefinitely()).unwrap();
                    if i > 1 {
                        timings.push(start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
            }
            timings.sort_by(f64::total_cmp);
            eprintln!(
                "32,768 particles, 720p, four modes: median {:.3} ms, p95 {:.3} ms (submit + GPU wait)",
                timings[timings.len() / 2],
                timings[timings.len() * 95 / 100]
            );
            for layer in &mut f.layers {
                layer.palette = [0.0; 4];
            }
            // Save one deterministic rendered frame for visual inspection, never an input capture.
            f.layout[3] = 0.0;
            step(&d, &q, &library, &mut g, f, &field, size);
            let pixels = pixels(&d, &q, &g);
            let visible = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0].max(p[1]).max(p[2]) > 20)
                .count();
            assert!(visible > 1000, "render must contain visible particles");
            let mut ppm = b"P6\n1280 720\n255\n".to_vec();
            for p in pixels.as_chunks::<4>().0.iter() {
                ppm.extend_from_slice(&p[..3]);
            }
            std::fs::write(std::env::temp_dir().join("symphos-particle-waves.ppm"), ppm).unwrap();
            assert!(scope.pop().await.is_none());
        });
    }
}
