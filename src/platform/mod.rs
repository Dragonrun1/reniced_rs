#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::is_privileged;

#[cfg(windows)]
pub use windows::is_privileged;
