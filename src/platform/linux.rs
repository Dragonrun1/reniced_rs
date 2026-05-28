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
