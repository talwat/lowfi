//! Helps create a lot of debug logs.

use std::sync::{atomic::{AtomicBool, Ordering}, Mutex};
use lazy_static::lazy_static;

static ENABLED: AtomicBool = AtomicBool::new(false);

lazy_static! {
    static ref LAST_MSG: Mutex<Option<(String, usize)>> = Mutex::new(None);
}

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        if $crate::dbg::is_enabled() {
            $crate::dbg::log(format!($($arg)*));
        }
    }};
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn log(message: String) {
    if !is_enabled() { return; }

    log::debug!("{}", message);
}

// I love this debug logs.