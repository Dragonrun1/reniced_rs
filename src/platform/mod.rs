#[cfg(unix)]
pub mod unix;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{is_privileged, set_process_priority};

#[cfg(windows)]
pub use windows::{is_privileged, set_io_priority, set_process_priority};
