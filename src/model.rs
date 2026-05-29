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

/// A rule defining priority adjustments for matching processes.
///
/// This struct contains a regular expression for process matching and optional
/// settings for CPU nice, OOM killer score, and I/O priority.
#[derive(Debug, Clone)]
pub struct Rule {
    /// The regular expression used to match against process attributes
    /// (name, cmdline, exe, etc.) as specified by `MatchTarget`.
    pub regex: Regex,
    /// The CPU scheduling priority (nice value) to apply, if set.
    /// - Range: -20 (highest) to 19 (lowest).
    pub nice: Option<i32>,
    /// The OOM (Out-of-Memory) adjustment score to apply, if set.
    /// - Range: -17 (immune) to 15 (most likely to be killed).
    pub oom_adj: Option<i32>,
    /// The I/O scheduling class to apply, if set.
    pub io_class: Option<IoClass>,
    /// The I/O priority level within the class, if set.
    /// - Range: 0 (highest) to 7 (lowest).
    pub io_nice: Option<u8>,
}

/// The I/O scheduling class for a process.
///
/// These classes determine the relative priority of a process's disk I/O operations.
/// The kernel's I/O scheduler uses this to allocate bandwidth during contention.
#[derive(Debug, Clone, Copy)]
pub enum IoClass {
    /// **Realtime**.
    ///
    /// Highest I/O priority. For critical tasks that must access the disk immediately.
    /// Use sparingly as it can starve other processes.
    Realtime,
    /// **Best Effort**.
    ///
    /// Default class for most processes. The `io_nice` level provides 8 priority levels.
    BestEffort,
    /// **Idle**.
    ///
    /// I/O only runs when no other process has pending disk operations.
    /// Suitable for background tasks like backups or indexing.
    Idle,
}

/// Represents a running process or thread.
///
/// Contains identifying information used for matching rules and applying adjustments.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    /// The Process ID (PID) or Thread ID (TID).
    pub pid: i32,
    /// The full command line used to start the process.
    pub cmd: String,
    /// The process name (e.g., `firefox`).
    /// On Linux, this is limited to 15 characters.
    pub name: String,
    /// The full path to the executable, if available.
    pub exe: Option<String>,
    /// Whether this entry represents a main process or a thread.
    pub kind: ProcessKind,
}

/// Distinguishes between a main process and its threads.
#[derive(Debug, Clone)]
pub enum ProcessKind {
    /// A main process.
    Process,
    /// A thread belonging to a process.
    Thread,
}
