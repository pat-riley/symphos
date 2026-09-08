//! Bounded diagnostic reports. Never called from the PipeWire callback.
use eframe::egui;
use std::{
    collections::VecDeque,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ENTRIES: usize = 200;
const MAX_FILE_BYTES: u64 = 1_048_576;
#[derive(Default)]
struct IssueLog {
    entries: VecDeque<String>,
    path: Option<PathBuf>,
}
static LOG: OnceLock<Mutex<IssueLog>> = OnceLock::new();
fn state() -> &'static Mutex<IssueLog> {
    LOG.get_or_init(|| Mutex::new(IssueLog::default()))
}

pub fn init() {
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
    if let Some(root) = root {
        let dir = root.join("symphos");
        if std::fs::create_dir_all(&dir).is_ok() {
            state().lock().unwrap_or_else(|e| e.into_inner()).path = Some(dir.join("issues.log"));
        }
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record("panic", &info.to_string());
        previous(info);
    }));
}

pub fn record(context: &str, message: &str) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let entry: String = format!("{seconds} · {context}: {message}")
        .chars()
        .take(1200)
        .collect();
    log::warn!("{entry}");
    let mut log = state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(path) = &log.path
        && std::fs::metadata(path).map_or(0, |m| m.len()) < MAX_FILE_BYTES
        && let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = writeln!(file, "{entry}");
    }
    log.entries.push_back(entry);
    while log.entries.len() > MAX_ENTRIES {
        log.entries.pop_front();
    }
}

pub fn count() -> usize {
    state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entries
        .len()
}
pub fn report() -> String {
    let log = state().lock().unwrap_or_else(|e| e.into_inner());
    format!(
        "Symphos {} · {} / {}\nSession issues (newest last):\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        log.entries.iter().cloned().collect::<Vec<_>>().join("\n")
    )
}

pub fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let modal = egui::Modal::new(egui::Id::new("issue-log")).show(ctx, |ui| {
        ui.set_width(540.0_f32.min(ctx.content_rect().width() - 48.0));
        ui.heading("Issue log");
        ui.label("Rejected parameter values keep the last valid setting. Nothing is sent to GitHub automatically.");
        let path = state().lock().unwrap_or_else(|e| e.into_inner()).path.clone();
        if let Some(path) = path { ui.small(format!("Log: {} (1 MiB cap)", path.display())); }
        let mut text = report();
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut text).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).interactive(false));
        });
        ui.horizontal(|ui| {
            if ui.button("Copy issue report").clicked() { ctx.copy_text(report()); }
            if ui.button("Clear session").clicked() { state().lock().unwrap_or_else(|e| e.into_inner()).entries.clear(); }
            if ui.button("Close").clicked() { *open = false; }
        });
    });
    if modal.should_close() {
        *open = false;
    }
}
