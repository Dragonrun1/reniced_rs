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

use crate::model::{ProcessEntry, ProcessKind};
use std::ffi::OsStr;
use sysinfo::{Pid, System};

/// Collects all threads belonging to a specific process into a list of [`ProcessEntry`]s.
///
/// This function uses the provided [`System`] snapshot to locate the main process by PID,
/// then iterates over its associated tasks (threads). On Linux, threads are treated as
/// distinct processes with their own PIDs (TIDs), which this function captures.
///
/// # Arguments
///
/// * `pid` - The PID of the main process to inspect.
/// * `system` - A reference to an up-to-date [`System`] snapshot containing process information.
///
/// # Returns
///
/// A `Vec<ProcessEntry>` where each entry represents a thread of the target process.
/// - If the main process is not found, or it has no tasks, an empty vector is returned.
/// - Each entry is marked with `ProcessKind::Thread`.
///
/// # Behavior
///
/// - **Name**: Derived from the thread's individual name (`thread.name()`).
/// - **Command Line**: Constructed by joining the thread's argument list with spaces.
/// - **Executable**: Mapped from the thread's executable path, if available.
/// - **Kind**: Explicitly set to [`ProcessKind::Thread`] to distinguish from the main process.
///
/// # Platform Specifics
///
/// - **Linux**: Threads appear as separate entries in the process table (TIDs). This function
///   effectively lists all TIDs associated with the main PID.
/// - **Windows/macOS**: The `tasks()` method behavior depends on `sysinfo`'s implementation.
///   On some platforms, threads may not be exposed as distinct `Process` objects, potentially
///   resulting in an empty vector.
///
/// # Example
///
/// ```no_run
/// # use sysinfo::{System, SystemExt};
/// # use your_crate::{collect_threads, ProcessEntry};
/// let mut system = System::new_all();
/// let pid = 1234;
///
/// let threads = collect_threads(pid, &system);
/// println!("Found {} threads", threads.len());
/// for thread in threads {
///     println!("Thread {} ({})", thread.pid, thread.name);
/// }
/// ```
///
/// # Note
///
/// The `system` argument must be refreshed (`system.refresh_all()`) before calling this function,
/// otherwise the task list may be empty or stale.
//noinspection DuplicatedCode
pub fn collect_threads(pid: i32, system: &System) -> Vec<ProcessEntry> {
    let sys_pid = Pid::from(pid as usize);
    let mut threads = Vec::new();

    if let Some(process) = system.process(sys_pid) {
        if let Some(task_pids) = process.tasks() {
            for task_pid in task_pids {
                if let Some(thread) = system.process(*task_pid) {
                    threads.push(ProcessEntry {
                        pid: task_pid.as_u32() as i32,
                        name: thread.name().to_string_lossy().into_owned(),
                        cmd: thread
                            .cmd()
                            .join(OsStr::new(" "))
                            .to_string_lossy()
                            .into_owned(),
                        exe: thread.exe().map(|p| p.to_string_lossy().into_owned()),
                        kind: ProcessKind::Thread,
                    });
                }
            }
        }
    }

    threads
}
