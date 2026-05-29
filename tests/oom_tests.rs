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

// convert_oom_adj is re-exported from actions for backward compat;
// the canonical source is platform::linux on Linux builds.
use reniced::actions::convert_oom_adj;

#[test]
fn converts_maximum_oom_adj() {
    // The legacy maximum maps to the new-interface maximum.
    assert_eq!(convert_oom_adj(15), 1000);
}

#[test]
fn converts_zero_oom_adj() {
    assert_eq!(convert_oom_adj(0), 0);
}

#[test]
fn converts_negative_oom_adj() {
    assert_eq!(convert_oom_adj(-17), -1000);
}

#[test]
fn converts_mid_range_positive() {
    // Formula: (score * 1000) / 17 => (7 * 1000) / 17 = 411 (integer division)
    assert_eq!(convert_oom_adj(7), 411);
}

#[test]
fn converts_mid_range_negative() {
    // (-8 * 1000) / 17 = -470 (integer division toward zero in Rust)
    assert_eq!(convert_oom_adj(-8), -470);
}

#[test]
fn converts_one() {
    assert_eq!(convert_oom_adj(1), 58);
}

#[test]
fn converts_minus_one() {
    assert_eq!(convert_oom_adj(-1), -58);
}
