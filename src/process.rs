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

use anyhow::Result;
use std::ffi::OsStr;
use sysinfo::System;

use crate::model::{ProcessEntry, ProcessKind};
use crate::platform::linux;

/// Reads a list of all currently running processes from the system.
///
/// This function creates a new `System` snapshot, refreshes all process information,
/// and collects the main processes. If `include_threads` is true and the target is Linux,
/// it also collects threads for each process.
///
/// # Arguments
///
/// * `include_threads` - If true, includes threads (TIDs) on Linux. Ignored on other platforms.
///
/// # Returns
///
/// * `Ok(Vec<ProcessEntry>)` containing all main processes, and threads if requested and supported.
/// * `Err(...)` if the `sysinfo` operations fail (rare).
pub fn read_processes(include_threads: bool) -> Result<Vec<ProcessEntry>> {
    let mut system = System::new_all();
    system.refresh_all();
    collect_entries(&system, include_threads)
}

/// Collects `ProcessEntry` instances from a `System` snapshot.
///
/// Iterates over all processes in the `system` snapshot and creates a `ProcessEntry`
/// for each main process. If `include_threads` is true and the target is Linux,
/// it calls `linux::collect_threads` to append thread entries.
///
/// # Arguments
///
/// * `system` - A reference to a `sysinfo::System` snapshot with refreshed data.
/// * `include_threads` - Flag to include threads (Linux only).
///
/// # Returns
///
/// A vector of `ProcessEntry` objects, each marked as either `ProcessKind::Process` or `ProcessKind::Thread`.
//noinspection DuplicatedCode
pub fn collect_entries(system: &System, include_threads: bool) -> Result<Vec<ProcessEntry>> {
    let mut entries = Vec::new();

    for (pid, process) in system.processes() {
        // Add the main process
        entries.push(ProcessEntry {
            pid: pid.as_u32() as i32,
            name: process.name().to_string_lossy().into_owned(),
            cmd: process
                .cmd()
                .join(OsStr::new(" "))
                .to_string_lossy()
                .into_owned(),
            exe: process.exe().map(|p| p.to_string_lossy().into_owned()),
            kind: ProcessKind::Process,
        });

        // Conditionally collect threads on Linux
        if include_threads {
            #[cfg(target_os = "linux")]
            {
                entries.extend(linux::collect_threads(pid.as_u32() as i32, &system));
            }
        }
    }

    Ok(entries)
}
