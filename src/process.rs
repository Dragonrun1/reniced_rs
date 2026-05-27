use anyhow::Result;
use sysinfo::System;

use crate::model::ProcessEntry;

pub fn read_processes(_include_threads: bool) -> Result<Vec<ProcessEntry>> {
    let mut system = System::new_all();
    system.refresh_all();
    let entries = system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessEntry {
            pid: pid.as_u32() as i32,
            cmd: process.name().to_string_lossy().into_owned(),
        })
        .collect();

    Ok(entries)
}
