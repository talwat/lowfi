//! Some simple audio related utilities.

pub mod waiter;

/// This gets the output stream while also shutting up alsa with [libc].
/// Uses raw libc calls, and therefore is functional only on Linux.
#[cfg(target_os = "linux")]
fn silent_get_output_stream() -> crate::Result<rodio::MixerDeviceSink> {
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
    };

    // Make the MixerDeviceSink while stderr is still redirected to /dev/null.
    let stream = rodio::DeviceSinkBuilder::open_default_sink()?;

    // Redirect back to the current terminal, so that other output isn't silenced.
    let tty = CString::new("/dev/tty")?;

    // SAFETY: See the first call to `freopen`.
    unsafe {
        freopen(tty.as_ptr(), mode.as_ptr(), stderr);
    };

    Ok(stream)
}

/// Creates an audio stream, doing so silently on Linux.
pub fn stream() -> crate::Result<rodio::MixerDeviceSink> {
    #[cfg(target_os = "linux")]
    let mut stream = silent_get_output_stream()?;
    #[cfg(not(target_os = "linux"))]
    let mut stream = rodio::DeviceSinkBuilder::open_default_sink()?;
    stream.log_on_drop(false);

    Ok(stream)
}
