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

use regex::Regex;

#[derive(Debug, Clone)]
pub struct Rule {
    pub regex: Regex,
    pub nice: Option<i32>,
    pub oom_adj: Option<i32>,
    pub io_class: Option<IoClass>,
    pub io_nice: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum IoClass {
    Realtime,
    BestEffort,
    Idle,
}

#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: i32,
    pub cmd: String,
    pub name: String,
    pub exe: Option<String>,
    pub kind: ProcessKind,
}

#[derive(Debug, Clone)]
pub enum ProcessKind {
    Process,
    Thread,
}
