#[cfg(target_os = "windows")]
pub fn is_privileged() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken, TOKEN_QUERY};

    unsafe {
        let mut token_handle = HANDLE::default();

        // SAFETY:
        // OpenProcessToken only writes to token_handle.
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();

        let mut size = 0u32;

        // SAFETY:
        // elevation is a properly initialized writable
        // TOKEN_ELEVATION buffer.
        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok()
            && elevation.TokenIsElevated != 0;

        // SAFETY:
        // token_handle was returned by OpenProcessToken.
        let _ = CloseHandle(token_handle);

        result
    }
}
