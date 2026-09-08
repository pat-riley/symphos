mod analysis;
mod app;
mod audio;
mod global_bar;
mod help;
mod icons;
mod issues;
mod modules;
mod parameter;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("symphos=info,wgpu_core=warn"),
    )
    .init();
    issues::init();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Symphos")
            .with_app_id("io.github.riley.Symphos")
            .with_inner_size([1440.0, 940.0])
            .with_min_inner_size([1024.0, 700.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Symphos",
        options,
        Box::new(|context| Ok(Box::new(app::SymphosApp::new(context)))),
    )
}

fn app_icon() -> egui::IconData {
    let size = 64_u32;
    let mut rgba = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let index = ((y * size + x) * 4) as usize;
            let dx = x as f32 - 31.5;
            let dy = y as f32 - 31.5;
            let inside = dx * dx + dy * dy <= 29.0 * 29.0;
            let wave = (y as f32 - 32.0 - (x as f32 * 0.34).sin() * 11.0).abs() < 2.4
                || (y as f32 - 32.0 - (x as f32 * 0.21 + 1.7).sin() * 17.0).abs() < 1.5;
            let (red, green, blue, alpha) = if inside && wave {
                (231, 236, 225, 255)
            } else if inside {
                (54, 161, 102, 255)
            } else {
                (0, 0, 0, 0)
            };
            rgba[index..index + 4].copy_from_slice(&[red, green, blue, alpha]);
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}
