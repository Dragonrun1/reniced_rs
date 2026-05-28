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

pub fn is_privileged() -> bool {
    // SAFETY:
    // geteuid() is thread-safe and has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

pub fn set_process_priority(
    pid: i32,
    nice: i32,
) -> io::Result<()> {
    // SAFETY:
    // set priority only uses primitive integer arguments.
    let result = unsafe {
        libc::setpriority(
            libc::PRIO_PROCESS,
            pid as libc::id_t,
            nice,
        )
    };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}