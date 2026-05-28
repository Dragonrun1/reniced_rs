//! Tests for logging::init.
//!
//! env_logger panics if initialised twice in the same process, so these tests
//! use log::max_level() to verify the level is set correctly after init.
//! Only init_stderr is testable here — init_system (syslog) requires a running
//! syslog socket and is integration-only.
//!
//! Tests are serialised via a Mutex to prevent races between log::set_max_level
//! calls from different threads.

use std::sync::Mutex;

use log::LevelFilter;
use reniced::cli::LogTarget;
use reniced::logging::init;

static LOG_INIT: Mutex<()> = Mutex::new(());

#[test]
fn stderr_non_verbose_sets_warn_level() {
    let _guard = LOG_INIT.lock().unwrap();
    init(&LogTarget::Stderr, false).unwrap();
    assert_eq!(log::max_level(), LevelFilter::Warn);
}

#[test]
fn stderr_verbose_sets_debug_level() {
    let _guard = LOG_INIT.lock().unwrap();
    init(&LogTarget::Stderr, true).unwrap();
    assert_eq!(log::max_level(), LevelFilter::Debug);
}
