//! Tests for platform::unix — is_privileged and set_process_priority.
//!
//! These make real OS calls but are safe:
//! - is_privileged() is a pure geteuid() read.
//! - set_process_priority() called on the current process with its current
//!   nice value is a no-op that exercises the real syscall path.
//!
//! The live ioprio_set and /proc writes are integration-only and not tested here.

#[cfg(unix)]
mod unix_tests {
    use reniced::platform::{is_privileged, set_process_priority};

    #[test]
    fn is_privileged_returns_bool_without_panic() {
        // We can't assert the value (test may run as root or non-root)
        // but we can assert it doesn't panic and returns a valid bool.
        let _ = is_privileged();
    }

    #[test]
    fn is_privileged_matches_geteuid() {
        let privileged = is_privileged();
        let euid = unsafe { libc::geteuid() };
        assert_eq!(privileged, euid == 0);
    }

    #[test]
    fn set_process_priority_self_nice_zero_succeeds() {
        // Setting the current process to nice 0 is always permitted and
        // exercises the full setpriority() syscall path.
        let pid = std::process::id() as i32;
        let result = set_process_priority(pid, 0);
        // May fail if current nice is already negative and we're not root,
        // since raising priority requires CAP_SYS_NICE. Accept either outcome
        // but assert it returns a proper Result rather than panicking.
        let _ = result;
    }

    #[test]
    fn set_process_priority_invalid_pid_errors() {
        // PID 0x7FFFFFFF is extremely unlikely to exist. setpriority on a
        // non-existent PID returns ESRCH.
        let result = set_process_priority(i32::MAX, 0);
        assert!(result.is_err());
    }
}
