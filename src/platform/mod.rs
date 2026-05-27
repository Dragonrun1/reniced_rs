#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
mod windows;
pub mod linux;

#[cfg(unix)]
pub use unix::{is_privileged, set_process_priority};

#[cfg(windows)]
pub use windows::{is_privileged, set_process_priority};
