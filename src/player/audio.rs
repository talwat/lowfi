#[cfg(target_os = "linux")]
use rodio::{OutputStream, OutputStreamHandle};

/// This gets the output stream while also shutting up alsa with [libc].
/// Uses raw libc calls, and therefore is functional only on Linux.
#[cfg(target_os = "linux")]
pub fn silent_get_output_stream() -> eyre::Result<(OutputStream, OutputStreamHandle)> {
    use libc::freopen;
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
    let (stream, handle) = OutputStream::try_default()?;

    // Redirect back to the current terminal, so that other output isn't silenced.
    let tty = CString::new("/dev/tty")?;

    // SAFETY: See the first call to `freopen`.
    unsafe {
        freopen(tty.as_ptr(), mode.as_ptr(), stderr);
    }

    Ok((stream, handle))
}
