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