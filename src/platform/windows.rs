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

// Windows IO priority levels passed to NtSetInformationProcess.
// These map onto the kernel's IO_PRIORITY_HINT enum:
//   IoPriorityVeryLow = 0  (background, used for IDLE class)
//   IoPriorityLow     = 1  (below-normal)
//   IoPriorityNormal  = 2  (default)
//   IoPriorityHigh    = 3  (requires elevated privileges)
//   IoPriorityCritical= 4  (reserved for OS, not exposed here)
#[derive(Clone, Copy)]
#[repr(u32)]
enum IoPriorityHint {
    VeryLow = 0,
    Low     = 1,
    Normal  = 2,
    High    = 3,
}

// ProcessInformationClass value for IO priority.
const PROCESS_IO_PRIORITY: u32 = 33;

#[allow(non_snake_case)]
// Signature of NtSetInformationProcess from ntdll.
type NtSetInformationProcessFn = unsafe extern "system" fn(
    ProcessHandle: HANDLE,
    ProcessInformationClass: u32,
    ProcessInformation: *mut core::ffi::c_void,
    ProcessInformationLength: u32,
) -> NTSTATUS;

// Resolve NtSetInformationProcess once and cache it. ntdll is guaranteed
// to be loaded in every Windows process so GetModuleHandle is sufficient —
// no LoadLibrary / FreeLibrary needed.
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

fn nice_to_windows_priority(nice: i32) -> u32 {
    match nice.clamp(-20, 19) {
        -20..=-16 => REALTIME_PRIORITY_CLASS.0,
        -15..=-9  => HIGH_PRIORITY_CLASS.0,
        -8..=-1   => ABOVE_NORMAL_PRIORITY_CLASS.0,
        0..=4     => NORMAL_PRIORITY_CLASS.0,
        5..=10    => BELOW_NORMAL_PRIORITY_CLASS.0,
        _         => IDLE_PRIORITY_CLASS.0,
    }
}

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
/// - `Realtime`   → `IoPriorityHigh`    (requires elevation; best available
///                                        equivalent for real-time workloads)
/// - `BestEffort` → `IoPriorityNormal`  (Windows default; the level value
///                                        from the rule is intentionally
///                                        ignored — there is no sub-class)
/// - `Idle`       → `IoPriorityVeryLow` (background IO, equivalent to
///                                        IDLE_PRIORITY_CLASS effect on IO)
///
/// `IoPriorityLow` is not exposed directly but is reachable in a future
/// extension if a finer mapping is needed.
pub fn set_io_priority(pid: i32, io_class: crate::model::IoClass) -> io::Result<()> {
    use crate::model::IoClass;

    let hint = match io_class {
        IoClass::Realtime   => IoPriorityHint::High,
        IoClass::BestEffort => IoPriorityHint::Normal,
        IoClass::Idle       => IoPriorityHint::VeryLow,
    };

    let nt_fn = get_nt_set_information_process()
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::NotFound,
            "NtSetInformationProcess not found in ntdll",
        ))?;

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
