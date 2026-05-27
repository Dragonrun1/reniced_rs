use std::io;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION};
use windows::Win32::System::Threading::{
    ABOVE_NORMAL_PRIORITY_CLASS,
    BELOW_NORMAL_PRIORITY_CLASS,
    GetCurrentProcess,
    HIGH_PRIORITY_CLASS,
    IDLE_PRIORITY_CLASS,
    NORMAL_PRIORITY_CLASS,
    OpenProcess,
    OpenProcessToken,
    PROCESS_QUERY_INFORMATION,
    PROCESS_SET_INFORMATION,
    REALTIME_PRIORITY_CLASS,
    SetPriorityClass,
    TOKEN_QUERY,
};

pub fn is_privileged() -> bool {
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

fn nice_to_windows_priority(nice: i32) -> u32 {
    match nice.clamp(-20, 19) {
        -20..=-16 => REALTIME_PRIORITY_CLASS.0,
        -15..=-9 => HIGH_PRIORITY_CLASS.0,
        -8..=-1 => ABOVE_NORMAL_PRIORITY_CLASS.0,
        0..=4 => NORMAL_PRIORITY_CLASS.0,
        5..=10 => BELOW_NORMAL_PRIORITY_CLASS.0,
        _ => IDLE_PRIORITY_CLASS.0,
    }
}

pub fn set_process_priority(pid: i32, nice: i32) -> io::Result<()> {
    let priority = nice_to_windows_priority(nice);

    unsafe {
        // SAFETY:
        // OpenProcess is called with a PID supplied
        // by process enumeration.
        let handle: HANDLE = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid as u32,
        )
        .map_err(io::Error::from)?;

        // SAFETY:
        // handle is a valid process handle returned
        // from OpenProcess.
        let result = SetPriorityClass(handle, priority);

        let _ = CloseHandle(handle);

        result.ok().map_err(io::Error::from)?;
    }

    Ok(())
}
