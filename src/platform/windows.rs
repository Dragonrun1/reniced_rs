// SPDX-FileCopyrightText: 2026 Michael Cummings <mgcummings@yahoo.com>
// SPDX-License-Identifier: GPL-2.0-or-later

// ///////////////////////////////////////////////////////////////////////////
// reniced_rs - A Rust library for renicing processes
//
// Copyright (C) 2026  Michael Cummings
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, see <https://www.gnu.org/licenses/>.
// ///////////////////////////////////////////////////////////////////////////

use std::io;
use std::sync::OnceLock;
use windows::Win32::Foundation::{CloseHandle, HANDLE, NTSTATUS, STATUS_SUCCESS};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, SetPriorityClass,
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
    IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
    REALTIME_PRIORITY_CLASS, TOKEN_QUERY,
};

/// Represents an I/O priority hint for the Windows I/O scheduler.
///
/// These values are used to suggest the relative importance of I/O operations
/// for a process or thread. The Windows kernel uses these hints to optimize
/// disk access scheduling, favoring higher priorities during contention.
///
/// This enum is designed for FFI interoperability with Windows APIs (e.g., `SetIoPriority`),
/// using `#[repr(u32)]` to ensure it matches the underlying `ULONG` expected by the system.
///
/// # Variants
///
/// * `VeryLow` (0): For background tasks that should only run when the system is idle.
///   Minimal impact on foreground responsiveness.
/// * `Low` (1): For non-critical background operations. Lower than normal applications.
/// * `Normal` (2): The default priority for most applications. Balanced I/O access.
/// * `High` (3): For time-critical operations. May preempt lower-priority I/O.
///
/// # Usage
///
/// Typically passed to undocumented NT APIs like `NtSetInformationProcess` with the
/// `ProcessIoPriority` information class, or used with `SetThreadIoPriority` (Windows 11+).
///
/// # Example
///
/// ```
/// # #[derive(Clone, Copy)]
/// # #[repr(u32)]
/// # enum IoPriorityHint { VeryLow = 0, Low = 1, Normal = 2, High = 3 }
/// let hint = IoPriorityHint::Normal;
/// assert_eq!(hint as u32, 2);
/// ```
#[derive(Clone, Copy)]
#[repr(u32)]
enum IoPriorityHint {
    VeryLow = 0,
    Low = 1,
    Normal = 2,
    High = 3,
}

// ProcessInformationClass value for IO priority.
const PROCESS_IO_PRIORITY: u32 = 33;

/// Type alias for the `NtSetInformationProcess` function pointer.
///
/// Signature: `fn(HANDLE, PROCESS_INFORMATION_CLASS, PVOID, ULONG) -> NTSTATUS`
#[allow(non_snake_case)]
type NtSetInformationProcessFn = unsafe extern "system" fn(
    ProcessHandle: HANDLE,
    ProcessInformationClass: u32,
    ProcessInformation: *mut core::ffi::c_void,
    ProcessInformationLength: u32,
) -> NTSTATUS;

/// Retrieves a function pointer to the undocumented `NtSetInformationProcess` API from `ntdll.dll`.
///
/// This function uses lazy initialization (`OnceLock`) to dynamically resolve the address of
/// `NtSetInformationProcess` at runtime. This is necessary because the function is not exported
/// in the standard Windows SDK import libraries and is considered an internal NT API.
///
/// # Mechanism
///
/// 1. **Module Handle**: Calls `GetModuleHandleW` to get a handle to the already-loaded `ntdll.dll`.
/// 2. **Symbol Resolution**: Calls `GetProcAddress` to find the address of `NtSetInformationProcess`.
/// 3. **Transmutation**: Safely transmutes the raw `FARPROC` pointer to the typed `NtSetInformationProcessFn`.
///
/// The result is cached globally after the first successful lookup. Subsequent calls return the
/// cached `Some` value immediately without system calls. If resolution fails (e.g., on a non-Windows NT system),
/// it returns `None` and caches the failure.
///
/// # Returns
///
/// * `Some(NtSetInformationProcessFn)` if the function was successfully resolved.
/// * `None` if `ntdll.dll` could not be accessed or the symbol was not found.
///
/// # Safety
///
/// This function contains `unsafe` blocks for the following reasons:
/// - **`GetModuleHandleW` / `GetProcAddress`**: Interactions with the OS loader require valid string pointers.
///   - `w!("ntdll")` is a valid, null-terminated wide string literal.
///   - `s!("NtSetInformationProcess")` is a valid, null-terminated ANSI string literal.
/// - **`std::mem::transmute`**: Converts a raw `FARPROC` (function pointer) to a typed Rust function pointer.
///   - This is safe **only if** the actual signature of `NtSetInformationProcess` in `ntdll.dll`
///     exactly matches the `NtSetInformationProcessFn` type definition.
///   - `NtSetInformationProcess` is a stable internal API across all modern Windows NT versions, making this assumption safe in practice.
///
/// # Usage
///
/// ```no_run
/// if let Some(nt_set_info) = get_nt_set_information_process() {
///     // SAFETY: We assume the caller provides a valid process handle and correct arguments
///     // matching the PROCESS_INFORMATION_CLASS being used.
///     let status = unsafe {
///         nt_set_info(
///             process_handle,
///             0x24, // Example: ProcessIoPriority
///             &mut priority as *mut _ as *mut _,
///             4,
///         )
///     };
///
///     if status.is_ok() {
///         println!("Success");
///     }
/// } else {
///     eprintln!("Failed to resolve NtSetInformationProcess");
/// }
/// ```
///
/// # References
///
/// - [Undocumented Windows NT API](https://undocumented.ntinternals.net/)
/// - `ntdll.dll` is a core component of the Windows NT kernel interface.
fn get_nt_set_information_process() -> Option<NtSetInformationProcessFn> {
    static FN_PTR: OnceLock<Option<NtSetInformationProcessFn>> = OnceLock::new();

    *FN_PTR.get_or_init(|| {
        unsafe {
            // SAFETY: "ntdll\0" is a valid null-terminated wide string literal.
            let module = GetModuleHandleW(windows::core::w!("ntdll")).ok()?;

            // SAFETY: module is a valid handle; the function name is a valid
            // null-terminated ASCII string.
            let proc = GetProcAddress(module, windows::core::s!("NtSetInformationProcess"))?;

            // SAFETY: the retrieved function pointer matches the declared
            // NtSetInformationProcessFn signature — this is a stable,
            // documented NT API.
            Some(std::mem::transmute::<_, NtSetInformationProcessFn>(proc))
        }
    })
}

/// Checks if the current process is running with elevated privileges (Administrator).
///
/// On Windows, this function queries the process access token to determine if the
/// user account has been elevated via User Account Control (UAC). It returns `true`
/// if the process is running as an administrator, and `false` otherwise (including
/// standard user accounts or non-elevated administrator accounts).
///
/// # Mechanism
///
/// 1. **Open Token**: Calls `OpenProcessToken` on the current process handle with `TOKEN_QUERY` access.
/// 2. **Query Elevation**: Calls `GetTokenInformation` with `TokenElevation` to retrieve the `TOKEN_ELEVATION` structure.
/// 3. **Check Flag**: Inspects the `TokenIsElevated` field. A non-zero value indicates elevation.
/// 4. **Cleanup**: Ensures the token handle is closed via `CloseHandle`.
///
/// # Returns
///
/// * `true` if the process is elevated (running as Administrator).
/// * `false` if:
///   - The process is not elevated.
///   - Any step fails (e.g., unable to open the token, `GetTokenInformation` fails).
///
/// # Safety
///
/// This function uses `unsafe` blocks for FFI calls:
/// - **`OpenProcessToken`**: Writes to `token_handle`. The handle is guaranteed to be valid if the call succeeds.
/// - **`GetTokenInformation`**: Requires a valid, initialized buffer (`TOKEN_ELEVATION`) and correct size.
///   - `elevation` is default-initialized, and the size passed matches `sizeof(TOKEN_ELEVATION)`.
/// - **`CloseHandle`**: Called only if `token_handle` was successfully opened.
///
/// These calls adhere to Windows API contracts, making the usage safe within this context.
///
/// # Platform
///
/// **Windows Only**. This function relies on the Windows Access Token API and UAC infrastructure.
///
/// # Example
///
/// ```no_run
/// # use your_crate::is_privileged;
/// if is_privileged() {
///     println!("Running as Administrator. Safe to modify system settings.");
/// } else {
///     println!("Running as standard user. Some operations may fail.");
/// }
/// ```
///
/// # Notes
///
/// - This check specifically verifies **UAC elevation**, not just membership in the Administrators group.
///   An administrator running without "Run as Administrator" will return `false`.
/// - If the process lacks permission to query its own token (rare), this function safely returns `false`.
pub fn is_privileged() -> bool {
    unsafe {
        let mut token_handle = HANDLE::default();

        // SAFETY: OpenProcessToken only writes to token_handle.
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;

        // SAFETY: elevation is a properly initialised writable TOKEN_ELEVATION buffer.
        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok()
            && elevation.TokenIsElevated != 0;

        // SAFETY: token_handle was returned by OpenProcessToken.
        let _ = CloseHandle(token_handle);

        result
    }
}

/// Converts a Linux "nice" value to a Windows priority class constant.
///
/// Linux nice values range from **-20** (highest priority) to **19** (lowest priority).
/// Windows uses discrete priority classes defined in [`windows::Win32::System::Threading`].
/// This function maps the continuous nice range to the nearest appropriate Windows priority class.
///
/// The input value is automatically clamped to the valid nice range `[-20, 19]` before mapping.
///
/// # Mapping Logic
///
/// | Nice Range | Windows Priority Class | Constant Value | Description |
/// | :--- | :--- | :--- | :--- |
/// | `-20` to `-16` | [`REALTIME_PRIORITY_CLASS`] | `0x00000100` | Highest priority. Can starve system threads; use with extreme caution. |
/// | `-15` to `-9`  | [`HIGH_PRIORITY_CLASS`] | `0x00000080` | For time-critical tasks. Preempts normal/idle processes. |
/// | `-8` to `-1`   | [`ABOVE_NORMAL_PRIORITY_CLASS`] | `0x00008000` | Higher than normal, but below high. |
/// | `0` to `4`     | [`NORMAL_PRIORITY_CLASS`] | `0x00000020` | Default priority for most processes. |
/// | `5` to `10`    | [`BELOW_NORMAL_PRIORITY_CLASS`] | `0x00004000` | Lower than normal, suitable for background tasks. |
/// | `11` to `19`   | [`IDLE_PRIORITY_CLASS`] | `0x00000040` | Lowest priority. Runs only when system is idle. |
///
/// # Arguments
///
/// * `nice` - The Linux nice value. Values outside `[-20, 19]` are clamped to the nearest boundary.
///
/// # Returns
///
/// A `u32` representing the corresponding Windows priority class constant (e.g., `0x00000100` for Realtime).
/// This return type is compatible with the `dwPriorityClass` parameter of [`SetPriorityClass`](windows::Win32::System::Threading::SetPriorityClass).
///
/// # Example
///
/// ```
/// # use windows::Win32::System::Threading::{NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS};
/// # fn nice_to_windows_priority(nice: i32) -> u32 {
/// #     match nice.clamp(-20, 19) {
/// #         -20..=-16 => 0x00000100, // REALTIME
/// #         -15..=-9  => 0x00000080, // HIGH
/// #         -8..=-1   => 0x00008000, // ABOVE_NORMAL
/// #         0..=4     => 0x00000020, // NORMAL
/// #         5..=10    => 0x00004000, // BELOW_NORMAL
/// #         _         => 0x00000040, // IDLE
/// #     }
/// # }
/// assert_eq!(nice_to_windows_priority(0), NORMAL_PRIORITY_CLASS.0);
/// assert_eq!(nice_to_windows_priority(-10), HIGH_PRIORITY_CLASS.0);
/// assert_eq!(nice_to_windows_priority(15), IDLE_PRIORITY_CLASS.0);
/// ```
///
/// # Safety Notes
///
/// Mapping to [`REALTIME_PRIORITY_CLASS`] (nice ≤ -16) can make the system unresponsive
/// if the process consumes significant CPU, as it preempts critical OS threads (mouse, keyboard, disk).
/// Use only for brief, critical operations.
///
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

/// Sets the scheduling priority class for a specific process on Windows.
///
/// This function converts a Linux-style `nice` value to a Windows priority class
/// using [`nice_to_windows_priority`], then applies it to the target process via
/// the `SetPriorityClass` API.
///
/// # Arguments
///
/// * `pid` - The Process ID (PID) of the target process.
/// * `nice` - The Linux-style nice value (-20 to 19).
///   - Mapped to Windows classes: `REALTIME`, `HIGH`, `ABOVE_NORMAL`, `NORMAL`, `BELOW_NORMAL`, or `IDLE`.
///
/// # Returns
///
/// * `Ok(())` if the priority class was successfully updated.
/// * `Err(io::Error)` if `OpenProcess` or `SetPriorityClass` fails.
///
/// # Errors
///
/// Common errors include:
/// - `ERROR_ACCESS_DENIED`: The caller lacks permission to modify the process (e.g., trying to modify a system process without admin rights).
/// - `ERROR_INVALID_PARAMETER`: The `pid` does not correspond to an active process.
/// - `ERROR_PARTIAL_COPY`: Sometimes occurs if the process terminates during the call.
///
/// # Safety
///
/// The `unsafe` block handles FFI calls:
/// - **`OpenProcess`**: Called with a PID obtained from process enumeration. The handle is requested with `PROCESS_QUERY_INFORMATION` and `PROCESS_SET_INFORMATION`.
/// - **`SetPriorityClass`**: Called with a valid handle returned by `OpenProcess`.
/// - **`CloseHandle`**: Ensures the handle is released immediately after use to prevent resource leaks.
///
/// # Platform
///
/// **Windows Only**. This function relies on the Windows Thread API (`SetPriorityClass`).
///
/// # Example
///
/// ```no_run
/// # use your_crate::set_process_priority;
/// # use std::io;
/// # fn main() -> io::Result<()> {
/// // Set process 1234 to "High" priority (mapped from nice -10)
/// set_process_priority(1234, -10)?;
/// # Ok(())
/// # }
/// ```
///
/// # Note
///
/// Unlike Linux `nice` values which are continuous, Windows uses discrete priority classes.
/// Multiple `nice` values may map to the same Windows class (e.g., both -10 and -15 map to `HIGH_PRIORITY_CLASS`).
pub fn set_process_priority(pid: i32, nice: i32) -> io::Result<()> {
    let priority = nice_to_windows_priority(nice);

    unsafe {
        // SAFETY: OpenProcess is called with a PID from process enumeration.
        let handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid as u32,
        )
        .map_err(io::Error::from)?;

        // SAFETY: handle is a valid process handle from OpenProcess.
        let result = SetPriorityClass(handle, priority);

        let _ = CloseHandle(handle);

        result.ok().map_err(io::Error::from)?;
    }

    Ok(())
}

/// Map the three ionice classes onto Windows IO priority hints.
///
/// - `Realtime`   → `IoPriorityHigh`    (requires elevation; the best available
///                                        equivalent for real-time workloads)
/// - `BestEffort` → `IoPriorityNormal`  (Windows default; the level value
///                                        from the rule is intentionally
///                                        ignored — there is no subclass)
/// - `Idle`       → `IoPriorityVeryLow` (background IO, equivalent to
///                                        IDLE_PRIORITY_CLASS effect on IO)
///
/// `IoPriorityLow` is not exposed directly but is reachable in a future
/// extension if a finer mapping is needed.
pub fn set_io_priority(pid: i32, io_class: crate::model::IoClass) -> io::Result<()> {
    use crate::model::IoClass;

    let hint = match io_class {
        IoClass::Realtime => IoPriorityHint::High,
        IoClass::BestEffort => IoPriorityHint::Normal,
        IoClass::Idle => IoPriorityHint::VeryLow,
    };

    let nt_fn = get_nt_set_information_process().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "NtSetInformationProcess not found in ntdll",
        )
    })?;

    unsafe {
        // SAFETY: OpenProcess is called with a PID from process enumeration.
        let handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid as u32,
        )
        .map_err(io::Error::from)?;

        let mut hint_value = hint as u32;

        // SAFETY:
        // - handle is a valid process handle.
        // - PROCESS_IO_PRIORITY (33) is the documented ProcessInformationClass
        //   for IO priority on Vista+.
        // - hint_value is a correctly sized u32 matching PROCESS_IO_PRIORITY's
        //   expected ProcessInformation type.
        let status = nt_fn(
            handle,
            PROCESS_IO_PRIORITY,
            &mut hint_value as *mut u32 as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );

        let _ = CloseHandle(handle);

        if status != STATUS_SUCCESS {
            return Err(io::Error::from_raw_os_error(status.0));
        }
    }

    Ok(())
}
