//! Keep native window chrome in sync with the in-app navigation surface.
use crate::theme::AppTheme;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn symphos_window_chrome(red: u8, green: u8, blue: u8, dark: bool);
}

pub fn apply(theme: &AppTheme) {
    #[cfg(target_os = "macos")]
    // SAFETY: the bridge checks the AppKit main thread and ignores headless
    // contexts. It caches the window and palette, updating only when needed.
    unsafe {
        let color = theme.navigation;
        symphos_window_chrome(color.r(), color.g(), color.b(), theme.dark_mode);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = theme;
}
