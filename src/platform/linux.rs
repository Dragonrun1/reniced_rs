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
                        kind: ProcessKind::Thread,
                    });
                }
            }
        }
    }

    threads
}
