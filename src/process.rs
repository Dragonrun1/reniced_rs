use anyhow::Result;
use std::ffi::OsStr;
use sysinfo::System;

use crate::model::{ProcessEntry, ProcessKind};

#[cfg(target_os = "linux")]
use crate::platform::linux;

pub fn read_processes(include_threads: bool) -> Result<Vec<ProcessEntry>> {
    let mut system = System::new_all();
    system.refresh_all();
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
