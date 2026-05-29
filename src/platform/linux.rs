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

use crate::model::{IoClass, ProcessEntry, ProcessKind};
use std::io;
use sysinfo::{Pid, System};

/// Collects all threads belonging to a specific process into a list of [`ProcessEntry`]s.
///
/// Uses [`ProcessEntry::from_sysinfo`] for construction, eliminating the duplication
/// that previously existed between this function and `process::collect_entries`.
///
/// Returns an empty vec if the PID is not found or has no tasks.
pub fn collect_threads(pid: i32, system: &System) -> Vec<ProcessEntry> {
    let sys_pid = Pid::from(pid as usize);
    let mut threads = Vec::new();

    if let Some(process) = system.process(sys_pid) {
        if let Some(task_pids) = process.tasks() {
            for task_pid in task_pids {
                if let Some(thread) = system.process(*task_pid) {
                    threads.push(ProcessEntry::from_sysinfo(
                        task_pid.as_u32() as i32,
                        thread,
                        ProcessKind::Thread,
                    ));
                }
            }
        }
    }

    threads
}

/// Constructs the packed priority value for the `ioprio_set` syscall.
///
/// The kernel encodes I/O priority as `(class << 13) | data`.
///
/// - **class**: I/O scheduling class (1 = Realtime, 2 = Best Effort, 3 = Idle).
/// - **data**: Priority level within the class (0–7, where 0 is highest).
fn ioprio_value(class: u16, data: u16) -> libc::c_int {
    ((class << 13) | data) as libc::c_int
}

/// Sets the I/O scheduling class and priority for a process via `ioprio_set`.
///
/// # Arguments
///
/// * `pid` - The target process ID.
/// * `io_class` - The I/O scheduling class to apply.
/// * `level` - Optional priority level (0–7). Defaults to 0 if `None`.
///
/// # Errors
///
/// Returns `Err` if the `ioprio_set` syscall fails. Common causes:
/// - `EPERM`: Caller lacks `CAP_SYS_NICE`.
/// - `ESRCH`: Process does not exist.
/// - `EINVAL`: Invalid class or level.
///
/// # Safety
///
/// Uses `libc::syscall` with `SYS_ioprio_set`. Arguments are validated by the
/// kernel; the only unsafe requirement is that `pid` comes from a trusted source
/// (process enumeration), which is guaranteed by the caller.
pub fn set_io_priority(pid: i32, io_class: IoClass, level: Option<u8>) -> io::Result<()> {
    let class_num: u16 = match io_class {
        IoClass::Realtime => 1,
        IoClass::BestEffort => 2,
        IoClass::Idle => 3,
    };

    let data = u16::from(level.unwrap_or(0));
    let prio = ioprio_value(class_num, data);

    const IOPRIO_WHO_PROCESS: libc::c_int = 1;

    // SAFETY: SYS_ioprio_set is a stable Linux syscall. pid comes from
    // process enumeration; class and level are validated above.
    let result = unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, pid, prio) };

    if result != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

/// Converts a legacy `oom_adj` value (-17..15) to a modern `oom_score_adj` value (-1000..1000).
///
/// The kernel formula is: `oom_score_adj = (oom_adj * 1000) / 17`
/// with the special case that the legacy maximum 15 maps exactly to 1000.
///
/// # Example
///
/// ```
/// # fn convert_oom_adj(score: i32) -> i32 {
/// #     if score == 15 { 1000 } else { (score * 1000) / 17 }
/// # }
/// assert_eq!(convert_oom_adj(15), 1000);
/// assert_eq!(convert_oom_adj(0), 0);
/// assert_eq!(convert_oom_adj(-17), -1000);
/// ```
pub fn convert_oom_adj(score: i32) -> i32 {
    const OOM_ADJUST_MAX: i32 = 15;
    const OOM_SCORE_ADJ_MAX: i32 = 1000;
    const OOM_DISABLE: i32 = -17;

    if score == OOM_ADJUST_MAX {
        OOM_SCORE_ADJ_MAX
    } else {
        (score * OOM_SCORE_ADJ_MAX) / -OOM_DISABLE
    }
}

/// Writes an OOM score adjustment to `/proc/[pid]/oom_score_adj`.
///
/// Converts the legacy `oom_adj` value first via [`convert_oom_adj`].
///
/// # Errors
///
/// Returns `Err` if the write fails. Common causes:
/// - `ENOENT`: Process no longer exists.
/// - `EACCES`/`EPERM`: Requires root to adjust another process's OOM score.
/// - `EINVAL`: Score out of range (prevented by `convert_oom_adj`).
pub fn adjust_oom(pid: i32, score: i32) -> io::Result<()> {
    let converted = convert_oom_adj(score);
    let path = format!("/proc/{pid}/oom_score_adj");
    std::fs::write(&path, format!("{converted}\n"))
        .map_err(|e| io::Error::new(e.kind(), format!("failed writing {path}: {e}")))?;
    Ok(())
}
