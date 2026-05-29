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

use libc;
use std::io;

/// Checks if the current process is running with root privileges (UID 0).
///
/// This function retrieves the **effective user ID (EUID)** of the calling process
/// using the `geteuid` syscall. If the EUID is `0`, the process is considered
/// privileged (running as root).
///
/// # Mechanism
///
/// - Calls `libc::geteuid()` to fetch the effective user ID.
/// - Compares the result against `0`.
///
/// # Returns
///
/// * `true` if the effective user ID is `0` (root).
/// * `false` otherwise.
///
/// # Safety
///
/// The `unsafe` block is required because `libc::geteuid` is an FFI call to the C standard library.
/// However, the call is safe in this context because:
/// - `geteuid` takes no arguments.
/// - It has no preconditions (always safe to call).
/// - It is thread-safe and does not modify global state.
///
/// # Platform
///
/// **Unix-like Systems Only** (Linux, macOS, BSD, etc.). This function relies on POSIX APIs
/// and will not compile on Windows without a compatibility layer (e.g., `libc` shim).
///
/// # Example
///
/// ```no_run
/// # use reniced::platform::is_privileged;
/// if is_privileged() {
///     println!("Running as root. Can modify system-wide settings.");
/// } else {
///     println!("Running as unprivileged user. Operations may be restricted.");
/// }
/// ```
///
/// # Note
///
/// This checks the **effective** UID, not the real UID. This means it correctly identifies
/// processes that have gained privileges via `setuid` binaries (e.g., `sudo`) even if the
/// original user was not root.
pub fn is_privileged() -> bool {
    // SAFETY:
    // geteuid() is thread-safe and has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

/// Sets the scheduling priority (nice value) for a specific process.
///
/// This function wraps the POSIX `setpriority` syscall to adjust the CPU scheduling priority
/// of the process identified by `pid`. The `nice` value typically ranges from **-20** (highest priority)
/// to **19** (lowest priority).
///
/// # Arguments
///
/// * `pid` - The Process ID (PID) of the target process.
/// * `nice` - The new nice value.
///   - Lower values (e.g., -20) increase priority.
///   - Higher values (e.g., 19) decrease priority.
///   - Setting a negative nice value usually requires root privileges.
///
/// # Returns
///
/// * `Ok(())` if the priority was successfully updated.
/// * `Err(io::Error)` if the syscall fails (returns -1).
///
/// # Errors
///
/// Common errors returned by the underlying syscall include:
/// - `EPERM`: The caller lacks the `CAP_SYS_NICE` capability (or is not root) and attempted to lower the nice value (increase priority).
/// - `ESRCH`: No process found with the specified `pid`.
/// - `EINVAL`: The `which` argument (here `PRIO_PROCESS`) is invalid (unlikely here).
/// - `EACCES`: The caller attempted to modify a process they do not own, without sufficient privileges.
///
/// # Safety
///
/// The `unsafe` block is required for the FFI call to `libc::setpriority`.
/// It is safe in this context because:
/// - All arguments (`PRIO_PROCESS`, `pid`, `nice`) are primitive integers.
/// - `setpriority` does not access user-provided pointers or mutable global state in an unsafe way.
///
/// # Platform
///
/// **Unix-like Systems Only** (Linux, macOS, BSD). This function relies on the POSIX `setpriority` API.
///
/// # Example
///
/// ```no_run
/// # use reniced::platform::set_process_priority;
/// # use std::io;
/// # fn main() -> io::Result<()> {
/// // Lower priority (nicer) for process 1234
/// set_process_priority(1234, 10)?;
///
/// // Raise priority (requires root)
/// // set_process_priority(1234, -5)?;
/// # Ok(())
/// # }
/// ```
///
/// # Note
///
/// The `setpriority` syscall returns `-1` on error, but `-1` can also be a valid return value
/// for `getpriority`. Since this function only *sets* the value, we strictly check for `-1`
/// to detect errors and retrieve `errno` via `io::Error::last_os_error()`.
pub fn set_process_priority(pid: i32, nice: i32) -> io::Result<()> {
    // SAFETY:
    // set priority only uses primitive integer arguments.
    let result = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}
