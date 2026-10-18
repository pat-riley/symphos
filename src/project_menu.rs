//! Native macOS application and File menus. Other desktops expose the same commands in the app menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Settings,
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn symphos_file_menu_install();
    fn symphos_file_menu_take() -> std::ffi::c_int;
    fn symphos_project_choose(
        save: std::ffi::c_int,
        initial: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_char;
    fn symphos_project_path_free(path: *mut std::ffi::c_char);
}
pub fn install() {
    #[cfg(target_os = "macos")]
    // SAFETY: eframe creates and updates this app on the AppKit main thread.
    unsafe {
        symphos_file_menu_install();
    }
}
pub fn take() -> Option<Action> {
    #[cfg(target_os = "macos")]
    // SAFETY: this only exchanges an atomic command code from the native menu.
    match unsafe { symphos_file_menu_take() } {
        1 => Some(Action::Open),
        2 => Some(Action::Save),
        3 => Some(Action::SaveAs),
        4 => Some(Action::New),
        5 => Some(Action::Settings),
        _ => None,
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}
#[cfg(target_os = "macos")]
pub fn choose(save: bool, initial: &str) -> Option<std::path::PathBuf> {
    let path = std::ffi::CString::new(initial).ok()?;
    // SAFETY: called on the main thread; input lives across the call. The bridge
    // returns null on cancel or an owned NUL-terminated UTF-8 allocation.
    let selected = unsafe { symphos_project_choose(i32::from(save), path.as_ptr()) };
    if selected.is_null() {
        return None;
    }
    let result = unsafe { std::ffi::CStr::from_ptr(selected) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: this is the exact allocation returned by the bridge, freed once.
    unsafe {
        symphos_project_path_free(selected);
    }
    Some(result.into())
}
