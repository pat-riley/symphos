//! AVFoundation stays on the encoder worker; the UI never waits for compression.
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
};
unsafe extern "C" {
    fn symphos_record_choose() -> *mut c_char;
    fn symphos_project_path_free(path: *mut c_char);
    fn symphos_record_open(
        path: *const c_char,
        width: i32,
        height: i32,
        fps: i32,
        rate: i32,
        error: *mut c_char,
        size: usize,
    ) -> *mut c_void;
    fn symphos_record_video(
        handle: *mut c_void,
        bytes: *const u8,
        seconds: f64,
        error: *mut c_char,
        size: usize,
    ) -> i32;
    fn symphos_record_audio(
        handle: *mut c_void,
        samples: *const f32,
        frames: i32,
        seconds: f64,
        error: *mut c_char,
        size: usize,
    ) -> i32;
    fn symphos_record_finish(
        handle: *mut c_void,
        seconds: f64,
        error: *mut c_char,
        size: usize,
    ) -> i32;
    fn symphos_record_cancel(handle: *mut c_void);
}
pub fn choose() -> Option<PathBuf> {
    unsafe {
        let p = symphos_record_choose();
        if p.is_null() {
            return None;
        }
        let result = PathBuf::from(CStr::from_ptr(p).to_string_lossy().into_owned());
        symphos_project_path_free(p);
        Some(result)
    }
}
pub struct Encoder {
    handle: *mut c_void,
    last: f64,
    frame_bytes: usize,
}
fn message(error: &[c_char]) -> String {
    unsafe {
        CStr::from_ptr(error.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}
impl Encoder {
    pub fn open(path: &Path, size: [u32; 2], fps: u32, rate: u32) -> Result<Self, String> {
        if size.iter().any(|v| !(64..=4096).contains(v) || v % 2 != 0)
            || size[0] * size[1] > 4096 * 2160
            || ![30, 60, 120].contains(&fps)
            || !(8000..=192000).contains(&rate)
        {
            return Err("Unsupported video dimensions, frame rate, or audio sample rate.".into());
        }
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let mut error = [0; 1024];
        let handle = unsafe {
            symphos_record_open(
                path.as_ptr(),
                size[0] as i32,
                size[1] as i32,
                fps as i32,
                rate as i32,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if handle.is_null() {
            Err(message(&error))
        } else {
            Ok(Self {
                handle,
                last: 0.0,
                frame_bytes: size[0] as usize * size[1] as usize * 4,
            })
        }
    }
    pub fn video(&mut self, bytes: &[u8], time: f64) -> Result<bool, String> {
        if bytes.len() != self.frame_bytes || !time.is_finite() || time < 0.0 {
            return Err("Invalid video frame size or timestamp.".into());
        }
        let mut error = [0; 1024];
        let result = unsafe {
            symphos_record_video(
                self.handle,
                bytes.as_ptr(),
                time,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if result < 0 {
            Err(message(&error))
        } else {
            if result > 0 {
                self.last = self.last.max(time);
            }
            Ok(result > 0)
        }
    }
    pub fn audio(&mut self, samples: &[[f32; 2]], time: f64) -> Result<bool, String> {
        if samples.is_empty() {
            return Ok(true);
        }
        if samples.len() > i32::MAX as usize || !time.is_finite() || time < 0.0 {
            return Err("Invalid audio buffer size or timestamp.".into());
        }
        let mut error = [0; 1024];
        let result = unsafe {
            symphos_record_audio(
                self.handle,
                samples.as_ptr().cast(),
                samples.len() as i32,
                time,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if result < 0 {
            Err(message(&error))
        } else {
            Ok(result > 0)
        }
    }
    pub fn finish(mut self, time: f64) -> Result<(), String> {
        if !time.is_finite() || time < 0.0 {
            return Err("Invalid recording end time.".into());
        }
        let mut error = [0; 1024];
        let handle = std::mem::replace(&mut self.handle, std::ptr::null_mut());
        let ok = unsafe {
            symphos_record_finish(
                handle,
                time.max(self.last + 0.001),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if ok == 0 {
            Err(message(&error))
        } else {
            Ok(())
        }
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                symphos_record_cancel(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_malformed_frames_before_crossing_native_boundary() {
        let mut writer = Encoder {
            handle: std::ptr::null_mut(),
            last: 0.0,
            frame_bytes: 64 * 64 * 4,
        };
        assert!(writer.video(&[0; 16], 0.0).is_err());
        assert!(writer.video(&vec![0; 64 * 64 * 4], f64::NAN).is_err());
        assert!(writer.audio(&[[0.0; 2]], -1.0).is_err());
        assert!(
            Encoder::open(
                Path::new("/tmp/invalid-movie.mov"),
                [u32::MAX, 1],
                60,
                48000
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "requires macOS hardware video encoder"]
    fn encodes_stereo_audio_and_video_at_requested_dimensions() {
        let path =
            std::env::temp_dir().join(format!("symphos-recording-test-{}.mov", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut writer = Encoder::open(&path, [320, 180], 60, 48000).unwrap();
        for frame in 0..60 {
            let time = f64::from(frame) / 60.0;
            let mut pixels = vec![0u8; 320 * 180 * 4];
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[40, 80, (frame * 4) as u8, 255]);
            }
            let audio: Vec<_> = (0..800)
                .map(|i| {
                    let t = (frame * 800 + i) as f32 / 48000.0;
                    [
                        (t * 440.0 * std::f32::consts::TAU).sin() * 0.2,
                        (t * 660.0 * std::f32::consts::TAU).sin() * 0.2,
                    ]
                })
                .collect();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !writer.video(&pixels, time).unwrap() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            while !writer.audio(&audio, time).unwrap() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        writer.finish(1.0).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 4000);
        eprintln!("Encoded test movie: {}", path.display());
    }
}
