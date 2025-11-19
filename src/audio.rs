use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::sleep,
    time::Duration,
};

use rodio::Sink;
use tokio::sync::mpsc;

/// This gets the output stream while also shutting up alsa with [libc].
/// Uses raw libc calls, and therefore is functional only on Linux.
#[cfg(target_os = "linux")]
pub fn silent_get_output_stream() -> eyre::Result<rodio::OutputStream, crate::Error> {
    use libc::freopen;
    use rodio::OutputStreamBuilder;
    use std::ffi::CString;

    // Get the file descriptor to stderr from libc.
    extern "C" {
        static stderr: *mut libc::FILE;
    }

    // This is a bit of an ugly hack that basically just uses `libc` to redirect alsa's
    // output to `/dev/null` so that it wont be shoved down our throats.

    // The mode which to redirect terminal output with.
    let mode = CString::new("w")?;

    // First redirect to /dev/null, which basically silences alsa.
    let null = CString::new("/dev/null")?;

    // SAFETY: Simple enough to be impossible to fail. Hopefully.
    unsafe {
        freopen(null.as_ptr(), mode.as_ptr(), stderr);
    }

    // Make the OutputStream while stderr is still redirected to /dev/null.
    let stream = OutputStreamBuilder::open_default_stream()?;

    // Redirect back to the current terminal, so that other output isn't silenced.
    let tty = CString::new("/dev/tty")?;

    // SAFETY: See the first call to `freopen`.
    unsafe {
        freopen(tty.as_ptr(), mode.as_ptr(), stderr);
    }

    Ok(stream)
}

static LISTEN: AtomicBool = AtomicBool::new(false);
pub fn playing(status: bool) {
    LISTEN.store(status, std::sync::atomic::Ordering::Relaxed);
}

pub fn waiter(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>) -> crate::Result<()> {
    loop {
        if Arc::strong_count(&sink) == 1 {
            break Ok(());
        }

        sleep(Duration::from_millis(100));
        sink.sleep_until_end();

        if LISTEN.load(std::sync::atomic::Ordering::Relaxed) {
            tx.blocking_send(crate::Message::Next)?;
        }
    }
}
