//! egui adapter for the UI-independent GPU visualizer library.
use eframe::egui_wgpu::{self, wgpu};
use std::sync::{Arc, Mutex};
use symphos_visuals::{Frame, Library, OrbitalGpu};

pub fn install(context: &eframe::CreationContext<'_>) {
    if let Some(state) = &context.wgpu_render_state {
        let limits = state.device.limits();
        if limits.max_compute_invocations_per_workgroup < 128
            || limits.max_storage_buffers_per_shader_stage < 4
        {
            log::warn!(
                "Orbital unavailable: adapter does not support the required compute/storage limits"
            );
            return;
        }
        // Shader compilation is lazy: existing CPU components don't pay startup cost.
        context.egui_ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("visual-output-format"), state.target_format)
        });
    }
}
#[derive(Clone, Default)]
pub struct GpuView(Arc<Mutex<Option<OrbitalGpu>>>);
impl GpuView {
    pub fn show(&self, ui: &egui::Ui, rect: egui::Rect, frame: Frame, count: u32, scale: f32) {
        let Some(format) =
            ui.data(|d| d.get_temp::<wgpu::TextureFormat>(egui::Id::new("visual-output-format")))
        else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Orbital requires a compute-capable GPU",
                egui::FontId::proportional(14.0),
                ui.visuals().text_color(),
            );
            return;
        };
        let pixels = rect.size() * ui.ctx().pixels_per_point();
        let size = if ui
            .data(|d| d.get_temp::<bool>(egui::Id::new("recording-output")))
            .unwrap_or(false)
        {
            [
                pixels.x.round().clamp(1.0, 4096.0) as u32,
                pixels.y.round().clamp(1.0, 4096.0) as u32,
            ]
        } else {
            symphos_visuals::target_size(pixels.x, pixels.y, scale)
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Draw {
                view: self.clone(),
                frame,
                count,
                size,
                format,
            },
        ));
    }
}
struct Draw {
    view: GpuView,
    frame: Frame,
    count: u32,
    size: [u32; 2],
    format: wgpu::TextureFormat,
}
impl egui_wgpu::CallbackTrait for Draw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if resources.get::<Library>().is_none() {
            resources.insert(Library::new(device, self.format));
        }
        let lib = resources.get::<Library>().unwrap();
        let mut state = self.view.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.as_ref().is_none_or(|s| s.count() != self.count) {
            *state = Some(OrbitalGpu::new(device, lib, self.count, self.size));
        }
        state
            .as_mut()
            .unwrap()
            .prepare(device, queue, encoder, lib, self.frame, self.size);
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(state) = self
            .view
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            state.paint(resources.get::<Library>().unwrap(), pass);
        }
    }
}
