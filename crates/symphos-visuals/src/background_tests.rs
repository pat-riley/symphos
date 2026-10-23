//! Verify real GPU compositing preserves the host's background under empty emission.
use super::*;
use bytemuck::Zeroable;
use wgpu as w;
#[test]
#[ignore = "requires a real compute GPU"]
fn orbital_empty_pixels_preserve_the_scene_background() {
    pollster::block_on(async {
        let instance = w::Instance::default();
        let adapter = instance
            .request_adapter(&w::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&w::DeviceDescriptor::default())
            .await
            .unwrap();
        let errors = device.push_error_scope(w::ErrorFilter::Validation);
        let lib = Library::new(&device, w::TextureFormat::Rgba8Unorm);
        let mut gpu = OrbitalGpu::new(&device, &lib, 128, [64, 64]);
        let mut frame = Frame::zeroed();
        frame.clock = [0.0, 0.0, 0.0, 1.0];
        frame.view = [1.0, 1.0, 0.0, 0.0];
        frame.camera = [8.5, 2.0, 0.0, 0.0];
        frame.axis_x = [1.0, 0.0, 0.0, 0.0];
        frame.axis_y = [0.0, 1.0, 0.0, 0.0];
        frame.axis_z = [0.0, 0.0, 1.0, 0.0];
        frame.orbs = [1.0, 1.3, 0.18, 0.0];
        frame.flock = [0.8, 1.1, 0.8, 1.8];
        frame.audio[3] = 4.5;
        frame.particle_style = [0.0, 0.0, 1.0, 0.5];
        frame.orb_style[2] = 1.0;
        frame.light[2] = 1.0;
        let texture = device.create_texture(&w::TextureDescriptor {
            label: Some("scene background compositing test"),
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
        let readback = device.create_buffer(&w::BufferDescriptor {
            label: None,
            size: 64 * 64 * 4,
            usage: w::BufferUsages::COPY_DST | w::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        gpu.prepare(&device, &queue, &mut encoder, &lib, frame, [64, 64]);
        {
            let mut pass = encoder.begin_render_pass(&w::RenderPassDescriptor {
                color_attachments: &[Some(w::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: w::Operations {
                        load: w::LoadOp::Clear(w::Color {
                            r: 0.125,
                            g: 0.25,
                            b: 0.375,
                            a: 1.0,
                        }),
                        store: w::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            gpu.paint(&lib, &mut pass);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            w::TexelCopyBufferInfo {
                buffer: &readback,
                layout: w::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            texture.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(w::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
        device.poll(w::PollType::wait_indefinitely()).unwrap();
        recv.recv().unwrap().unwrap();
        let bytes = readback.slice(..).get_mapped_range().unwrap();
        for pixel in bytes.as_chunks::<4>().0 {
            assert!(
                pixel
                    .iter()
                    .zip([32u8, 64, 96, 255])
                    .all(|(&actual, expected)| actual.abs_diff(expected) <= 1),
                "background replaced: {pixel:?}"
            );
        }
        drop(bytes);
        readback.unmap();
        assert!(errors.pop().await.is_none());
    });
}
