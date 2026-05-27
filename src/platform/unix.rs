#[cfg(unix)]
pub fn is_privileged() -> bool {
    // SAFETY:
    // geteuid() is thread-safe and has no preconditions.
    unsafe { libc::geteuid() == 0 }
}
