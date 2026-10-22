use eframe::egui_wgpu::{self, wgpu};
use std::sync::{Arc, Mutex};
use symphos_visuals::cymatics_gpu::{CymaticsFrame, CymaticsGpu, CymaticsLibrary};
#[derive(Clone, Default)]
pub(super) struct View(Arc<Mutex<Option<CymaticsGpu>>>);
impl View {
    pub fn show(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        frame: CymaticsFrame,
        field: Arc<Vec<[f32; 4]>>,
        count: u32,
    ) {
        let Some(format) =
            ui.data(|d| d.get_temp::<wgpu::TextureFormat>(egui::Id::new("visual-output-format")))
        else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Cymatics requires a compute-capable GPU",
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
            symphos_visuals::target_size(pixels.x, pixels.y, 1.0)
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Draw {
                view: self.clone(),
                frame,
                field,
                count,
                size,
                format,
            },
        ));
    }
}
struct Draw {
    view: View,
    frame: CymaticsFrame,
    field: Arc<Vec<[f32; 4]>>,
    count: u32,
    size: [u32; 2],
    format: wgpu::TextureFormat,
}
impl egui_wgpu::CallbackTrait for Draw {
    fn prepare(
        &self,
        d: &wgpu::Device,
        _: &wgpu::Queue,
        _: &egui_wgpu::ScreenDescriptor,
        e: &mut wgpu::CommandEncoder,
        r: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if r.get::<CymaticsLibrary>().is_none() {
            r.insert(CymaticsLibrary::new(d, self.format));
        }
        let l = r.get::<CymaticsLibrary>().unwrap();
        let mut state = self.view.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.as_ref().is_none_or(|s| s.count != self.count) {
            *state = Some(CymaticsGpu::new(d, l, self.count, self.size));
        }
        state
            .as_mut()
            .unwrap()
            .prepare(d, e, l, self.frame, &self.field, self.size);
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        p: &mut wgpu::RenderPass<'static>,
        r: &egui_wgpu::CallbackResources,
    ) {
        if let Some(s) = self
            .view
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            s.paint(r.get::<CymaticsLibrary>().unwrap(), p);
        }
    }
}
