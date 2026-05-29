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

#[cfg(unix)]
pub mod unix;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{is_privileged, set_process_priority};

#[cfg(target_os = "linux")]
pub use linux::{adjust_oom, convert_oom_adj, set_io_priority};

#[cfg(windows)]
pub use windows::{is_privileged, set_io_priority, set_process_priority};
