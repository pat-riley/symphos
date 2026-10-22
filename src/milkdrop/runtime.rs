//! Bounded, asynchronous GL renderer. Never touches the audio callback or AppKit.
use super::preset::Preset;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    sync::{Arc, Mutex, mpsc},
    time::Instant,
};
unsafe extern "C" {
    fn symphos_milk_create(error: *mut c_char, size: usize) -> *mut c_void;
    fn symphos_milk_destroy(ptr: *mut c_void);
    fn symphos_milk_warnings(ptr: *mut c_void, out: *mut c_char, size: usize);
    fn symphos_milk_load(
        ptr: *mut c_void,
        source: *const c_char,
        textures: *const c_char,
        smooth: i32,
        seconds: f32,
        error: *mut c_char,
        size: usize,
    ) -> i32;
    fn symphos_milk_render(
        ptr: *mut c_void,
        w: u32,
        h: u32,
        time: f64,
        fps: u32,
        pcm: *const f32,
        count: usize,
        controls: *const f32,
        sensitivity: f32,
        mesh: u32,
        rgba: *mut u8,
        error: *mut c_char,
        size: usize,
    ) -> i32;
}
struct Engine(*mut c_void);
impl Engine {
    fn new() -> Result<Self, String> {
        let mut err = [0; 4096]; // SAFETY: writable error buffer; handle remains on the creating thread.
        let p = unsafe { symphos_milk_create(err.as_mut_ptr(), err.len()) };
        if p.is_null() {
            Err(error(&err))
        } else {
            Ok(Self(p))
        }
    }
    fn load(
        &mut self,
        p: &Preset,
        dir: &std::path::Path,
        smooth: bool,
        seconds: f32,
    ) -> Result<(), String> {
        p.validate()?;
        p.materialize(dir)?;
        let source = CString::new(p.source.as_str()).map_err(|e| e.to_string())?;
        let dir = CString::new(dir.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let mut err = [0; 4096];
        // SAFETY: live handle and buffers owned on this thread; C++ catches exceptions.
        if unsafe {
            symphos_milk_load(
                self.0,
                source.as_ptr(),
                dir.as_ptr(),
                smooth as i32,
                seconds,
                err.as_mut_ptr(),
                err.len(),
            )
        } == 0
        {
            Err(error(&err))
        } else {
            Ok(())
        }
    }
    fn warnings(&self) -> String {
        let mut buffer = [0; 4096];
        // SAFETY: handle is live on its owning thread; buffer is writable and NUL terminated.
        unsafe { symphos_milk_warnings(self.0, buffer.as_mut_ptr(), buffer.len()) };
        error(&buffer)
    }
    fn render(&mut self, r: &Request) -> Result<Vec<u8>, String> {
        let mut rgba = vec![0; r.size[0] as usize * r.size[1] as usize * 4];
        let mut err = [0; 4096];
        // SAFETY: buffers have exactly the advertised dimensions/count; fixed controls ABI is 32 floats.
        let ok = unsafe {
            symphos_milk_render(
                self.0,
                r.size[0],
                r.size[1],
                r.time,
                r.fps.clamp(1, 240),
                r.pcm.as_ptr(),
                r.pcm.len() / 2,
                r.controls.as_ptr(),
                r.sensitivity,
                r.mesh,
                rgba.as_mut_ptr(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        if ok == 0 { Err(error(&err)) } else { Ok(rgba) }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: called once on the GL-owning thread, after the last render.
        unsafe { symphos_milk_destroy(self.0) }
    }
}
fn error(buffer: &[c_char]) -> String {
    // SAFETY: native bridge always NUL terminates nonempty buffers.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}
#[derive(Clone)]
pub struct Request {
    pub preset: Arc<Preset>,
    pub size: [u32; 2],
    pub time: f64,
    pub fps: u32,
    pub pcm: Vec<f32>,
    pub controls: [f32; 32],
    pub sensitivity: f32,
    pub mesh: u32,
    pub transition: f32,
    pub restart: u64,
}
pub struct Output {
    pub image: Result<egui::ColorImage, String>,
    pub preset: Option<Arc<Preset>>,
    pub milliseconds: f32,
    pub warning: String,
}
pub struct Worker {
    input: Option<mpsc::SyncSender<Request>>,
    pub output: Arc<Mutex<Option<Output>>>,
}
impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let output = Arc::new(Mutex::new(None));
        let output_worker = output.clone();
        std::thread::Builder::new()
            .name("milkdrop-render".into())
            .spawn(move || {
                let mut engine = match Engine::new() {
                    Ok(e) => e,
                    Err(e) => {
                        *output_worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(Output {
                            image: Err(e),
                            preset: None,
                            milliseconds: 0.,
                            warning: String::new(),
                        });
                        ctx.request_repaint();
                        return;
                    }
                };
                let dir = std::env::temp_dir().join(format!(
                    "symphos-milkdrop-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ));
                let mut last: Option<Arc<Preset>> = None;
                let mut restart = 0;
                let mut loaded = false;
                let mut asset_dirs = std::collections::VecDeque::new();
                let mut serial = 0u64;
                while let Ok(r) = rx.recv() {
                    let start = Instant::now();
                    let changed = last.as_ref().is_none_or(|p| {
                        !Arc::ptr_eq(p, &r.preset)
                            && (p.source != r.preset.source || p.textures != r.preset.textures)
                    }) || restart != r.restart;
                    if changed {
                        // Use a fresh asset directory for each preset so stale textures cannot leak.
                        serial += 1;
                        let assets = dir.join(serial.to_string());
                        asset_dirs.push_back(assets.clone());
                        if asset_dirs.len() > 2 {
                            let _ = std::fs::remove_dir_all(asset_dirs.pop_front().unwrap());
                        }
                        let result = engine.load(
                            &r.preset,
                            &assets,
                            loaded && restart == r.restart,
                            r.transition,
                        );
                        restart = r.restart;
                        last = Some(r.preset.clone());
                        if let Err(e) = result {
                            loaded = false;
                            *output_worker.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(Output {
                                    image: Err(e),
                                    preset: Some(r.preset.clone()),
                                    milliseconds: start.elapsed().as_secs_f32() * 1000.,
                                    warning: engine.warnings(),
                                });
                            ctx.request_repaint();
                            continue;
                        }
                        loaded = true;
                    }
                    last = Some(r.preset.clone());
                    if !loaded {
                        continue;
                    }
                    let image = engine.render(&r).map(|rgba| {
                        egui::ColorImage::from_rgba_unmultiplied(
                            [r.size[0] as usize, r.size[1] as usize],
                            &rgba,
                        )
                    });
                    *output_worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(Output {
                        image,
                        preset: Some(r.preset.clone()),
                        milliseconds: start.elapsed().as_secs_f32() * 1000.,
                        warning: engine.warnings(),
                    });
                    ctx.request_repaint();
                }
                drop(engine);
                let _ = std::fs::remove_dir_all(dir);
            })
            .expect("start MilkDrop renderer");
        Self {
            input: Some(tx),
            output,
        }
    }
    pub fn submit(&self, request: Request) {
        if let Some(tx) = &self.input {
            let _ = tx.try_send(request);
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.input.take();
    }
}

/// A chronological ~23 ms stereo window at MilkDrop's reference 44.1 kHz.
/// Never concatenate overlapping analysis histories; each render receives one window.
pub fn audio_window(frame: &crate::analysis::AnalysisFrame, live: bool, gain: f32) -> Vec<f32> {
    let mut pcm = vec![0.; 1024 * 2];
    if !live || frame.sample_rate == 0 {
        return pcm;
    }
    let len = frame.waveform_left.len().min(frame.waveform_right.len());
    if len < 2 {
        return pcm;
    }
    let step = frame.sample_rate as f64 / 44100.;
    for (i, pair) in pcm.as_chunks_mut::<2>().0.iter_mut().enumerate() {
        let pos = (len - 1) as f64 - (1023 - i) as f64 * step;
        if pos < 0. {
            continue;
        }
        let a = pos.floor() as usize;
        let b = (a + 1).min(len - 1);
        let f = pos.fract() as f32;
        for (out, source) in pair
            .iter_mut()
            .zip([&frame.waveform_left, &frame.waveform_right])
        {
            let v = (source[a] + (source[b] - source[a]) * f) * gain;
            *out = if v.is_finite() { v.clamp(-1., 1.) } else { 0. };
        }
    }
    pcm
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(preset: Preset) -> Request {
        Request {
            preset: Arc::new(preset),
            size: [640, 360],
            time: 0.,
            fps: 60,
            pcm: (0..2048).map(|i| (i as f32 * 0.17).sin() * 0.3).collect(),
            controls: [0.; 32],
            sensitivity: 1.,
            mesh: 48,
            transition: 0.2,
            restart: 0,
        }
    }
    fn lit(image: &[u8]) -> usize {
        image
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8)
            .count()
    }
    #[test]
    #[ignore = "requires a real OpenGL GPU context"]
    fn native_preset_fps_tracks_output_target() {
        let dir = std::env::temp_dir().join(format!("symphos-milkdrop-fps-{}", std::process::id()));
        let mut engine = Engine::new().unwrap();
        let mut preset = super::super::preset::builtins().remove(0);
        preset.set_scalar("fDecay", 0.);
        preset.set_scalar("fVideoEchoAlpha", 0.);
        preset.set_scalar("ob_a", 0.);
        preset
            .source
            .push_str("\nper_frame_3=wave_a=above(fps,90);\n");
        let mut r = request(preset);
        engine.load(&r.preset, &dir, false, 0.).unwrap();
        assert_eq!(
            lit(&engine.render(&r).unwrap()),
            0,
            "60 FPS must reach preset equations"
        );
        r.fps = 120;
        r.time += 1. / 120.;
        assert!(
            lit(&engine.render(&r).unwrap()) > 100,
            "120 FPS must reach preset equations"
        );
        r.fps = 30;
        r.time += 1. / 30.;
        assert_eq!(
            lit(&engine.render(&r).unwrap()),
            0,
            "changing target must update existing engines"
        );
        drop(engine);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore = "requires a real OpenGL GPU context"]
    fn native_controls_transitions_resize_and_error_recovery() {
        // Run on separate threads just like separate components, with overlapping GL lifetimes.
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles = (0..2)
            .map(|index| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let dir = std::env::temp_dir().join(format!(
                        "symphos-milkdrop-lifecycle-{}-{index}",
                        std::process::id()
                    ));
                    let mut engine = Engine::new().unwrap();
                    barrier.wait();
                    let mut preset = super::super::preset::builtins().remove(0);
                    preset.set_scalar("fDecay", 0.);
                    preset.set_scalar("fVideoEchoAlpha", 0.);
                    preset.set_scalar("ob_a", 0.);
                    let mut r = request(preset);
                    engine.load(&r.preset, &dir, false, 0.).unwrap();
                    let baseline = engine.render(&r).unwrap();
                    assert!(lit(&baseline) > 100);
                    // Same time/audio/framebuffer: only the live opacity adjustment changes.
                    r.controls[11] = -1.;
                    let muted = engine.render(&r).unwrap();
                    assert_eq!(lit(&muted), 0, "live waveform opacity must reach zero");
                    r.controls[11] = 0.;
                    assert!(lit(&engine.render(&r).unwrap()) > 100);

                    // Negative zoom is a mirrored feedback transform in legacy
                    // presets, not a NaN texture coordinate (GLSL pow(-x, 1)).
                    let mut mirrored = (*r.preset).clone();
                    mirrored.set_scalar("fDecay", 1.);
                    mirrored
                        .source
                        .push_str("\nper_frame_3=zoom=-0.98;zoomexp=1;wave_a=0;\n");
                    engine.load(&mirrored, &dir, true, 0.).unwrap();
                    r.time += 1. / 60.;
                    let mirrored_image = engine.render(&r).unwrap();
                    assert!(
                        lit(&mirrored_image) > 100,
                        "negative zoom must preserve incoming feedback"
                    );

                    let mut reactive = (*r.preset).clone();
                    reactive
                        .source
                        .push_str("\nper_frame_3=wave_a=min(1,bass);\n");
                    engine.load(&reactive, &dir, false, 0.).unwrap();
                    r.sensitivity = 0.;
                    assert_eq!(lit(&engine.render(&r).unwrap()), 0);
                    r.sensitivity = 1.;
                    assert!(
                        lit(&engine.render(&r).unwrap()) > 100,
                        "beat sensitivity must reach preset equations"
                    );

                    // Empty/malformed shaders must be reported, then a valid preset can recover.
                    let mut invalid = super::super::preset::builtins().remove(2);
                    invalid.source = invalid.source.replace(
                        "ret = tex2D(sampler_main, p).xyz;",
                        "ret = unknown_function(uv);",
                    );
                    assert!(
                        engine
                            .load(&invalid, &dir, false, 0.)
                            .unwrap_err()
                            .contains("shader")
                    );
                    engine.load(&r.preset, &dir, false, 0.).unwrap();
                    assert!(lit(&engine.render(&r).unwrap()) > 100);
                    for (i, p) in super::super::preset::builtins().into_iter().enumerate() {
                        engine.load(&p, &dir, true, 0.15).unwrap();
                        r.preset = Arc::new(p);
                        r.size = [[960, 540], [1280, 720], [320, 480]][i];
                        let start = Instant::now();
                        let mut image = Vec::new();
                        for _ in 0..30 {
                            r.time += 1. / 60.;
                            image = engine.render(&r).unwrap();
                        }
                        assert_eq!(image.len(), r.size[0] as usize * r.size[1] as usize * 4);
                        assert!(lit(&image) > 100);
                        assert!(image.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
                        println!(
                            "Worker {index}, {:?}: {:.2} ms/frame including transition/readback",
                            r.size,
                            start.elapsed().as_secs_f64() * 1000. / 30.
                        );
                    }
                    drop(engine);
                    std::fs::remove_dir_all(dir).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }
    }
    #[test]
    fn audio_window_keeps_stereo_and_silences_disconnect() {
        let f = crate::analysis::AnalysisFrame {
            waveform_left: vec![0.25; 2400],
            waveform_right: vec![-0.5; 2400],
            ..Default::default()
        };
        let pcm = audio_window(&f, true, 2.);
        assert!(pcm.as_chunks::<2>().0.iter().all(|p| *p == [0.5, -1.]));
        assert!(audio_window(&f, false, 2.).iter().all(|v| *v == 0.));
    }
    #[test]
    #[ignore = "requires a real OpenGL GPU context"]
    fn native_presets_render_and_live_controls_change_pixels() {
        let dir = std::env::temp_dir().join("symphos-milkdrop-test");
        let mut e = Engine::new().unwrap();
        let mut presets = super::super::preset::builtins();
        if let Ok(path) = std::env::var("SYMPHOS_MILKDROP_TEST_TEXTURES") {
            let mut assets = presets[0].clone();
            assets.import_textures(std::path::Path::new(&path)).unwrap();
            assets.materialize(&dir).unwrap();
        }
        if let Ok(path) = std::env::var("SYMPHOS_MILKDROP_TEST_PRESETS") {
            let mut files = Vec::new();
            fn collect(p: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
                if p.is_dir() {
                    for e in std::fs::read_dir(p).unwrap().flatten() {
                        collect(&e.path(), files);
                    }
                } else if p.extension().is_some_and(|s| s == "milk") {
                    files.push(p.to_owned());
                }
            }
            collect(std::path::Path::new(&path), &mut files);
            files.sort();
            for path in files.into_iter() {
                presets.push(super::super::preset::Preset::import(&path).unwrap());
            }
        }
        let mut rendered = 0;
        let mut clock = 0.;
        let mut dark = Vec::new();
        let mut failures = Vec::new();
        for (preset_index, p) in presets.into_iter().enumerate() {
            let name = p.name.clone();
            println!("Loading {name}");
            match e.load(&p, &dir, false, 0.2) {
                Ok(()) => {}
                Err(err) => {
                    failures.push(format!("{name}: {err}"));
                    continue;
                }
            }
            let mut r = Request {
                preset: Arc::new(p),
                size: [640, 360],
                time: 0.,
                fps: 60,
                pcm: (0..2048).map(|i| (i as f32 * 0.17).sin() * 0.3).collect(),
                controls: [0.; 32],
                sensitivity: 1.,
                mesh: 48,
                transition: 0.5,
                restart: 0,
            };
            let start = Instant::now();
            let mut last = Vec::new();
            for _ in 0..30 {
                clock += 1. / 60.;
                r.time = clock;
                match e.render(&r) {
                    Ok(p) => last = p,
                    Err(err) => {
                        failures.push(format!("{name}: {err}"));
                        break;
                    }
                }
            }
            let colored = lit(&last);
            println!(
                "{name}: {:.2}ms/frame, {colored} lit pixels",
                start.elapsed().as_secs_f64() * 1000. / 30.
            );
            if colored == 0 {
                for _ in 0..150 {
                    clock += 1. / 60.;
                    r.time = clock;
                    last = e.render(&r).unwrap();
                }
                if lit(&last) == 0 {
                    // Feedback-only presets can depend on the outgoing image.
                    let seed = super::super::preset::builtins().remove(0);
                    e.load(&seed, &dir, false, 0.).unwrap();
                    for _ in 0..30 {
                        clock += 1. / 60.;
                        r.time = clock;
                        e.render(&r).unwrap();
                    }
                    e.load(&r.preset, &dir, true, 0.1).unwrap();
                    for _ in 0..60 {
                        clock += 1. / 60.;
                        r.time = clock;
                        last = e.render(&r).unwrap();
                    }
                    if lit(&last) == 0 {
                        // A steady tone never crosses volume gates above its running
                        // average. Also exercise transients before calling a preset dark.
                        let mut peak_lit = 0;
                        for beat_frame in 0..180 {
                            clock += 1. / 60.;
                            r.time = clock;
                            let gain = if beat_frame % 30 < 5 { 0.9 } else { 0.015 };
                            for (i, value) in r.pcm.iter_mut().enumerate() {
                                *value = (i as f32 * 0.17).sin() * gain;
                            }
                            last = e.render(&r).unwrap();
                            peak_lit = peak_lit.max(lit(&last));
                        }
                        println!("Transient input {name}: {peak_lit} peak lit pixels");
                        if peak_lit == 0 {
                            dark.push(name.clone());
                        }
                    }
                    println!(
                        "Extended {name}: {} lit pixels after incoming feedback",
                        lit(&last)
                    );
                } else {
                    println!("Extended {name}: {} lit pixels after 3 seconds", lit(&last));
                }
            }
            if preset_index < 3 {
                assert!(colored > 100, "black bundled preset: {name}");
            }
            if rendered == 0 {
                let baseline = last.clone();
                r.controls[4] = 1.;
                r.controls[0] = 0.2;
                r.controls[1] = 0.1;
                clock += 1. / 60.;
                r.time = clock;
                assert_ne!(e.render(&r).unwrap(), baseline);
                if let Ok(path) = std::env::var("SYMPHOS_MILKDROP_PREVIEW") {
                    let mut bytes = b"P6\n640 360\n255\n".to_vec();
                    for row in last.as_chunks::<{ 640 * 4 }>().0.iter().rev() {
                        for p in row.as_chunks::<4>().0.iter() {
                            bytes.extend_from_slice(&p[..3]);
                        }
                    }
                    std::fs::write(path, bytes).unwrap();
                }
            }
            rendered += 1;
        }
        println!(
            "Rendered {rendered}; rejected {}: {failures:#?}",
            failures.len()
        );
        println!("Still dark after extended/feedback check: {dark:#?}");
        assert!(rendered >= 3);
        assert!(failures.is_empty(), "Compatibility errors");
        let _ = std::fs::remove_dir_all(dir);
    }
}
