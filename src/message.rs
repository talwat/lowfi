/// Handles communication between different parts of the program.
#[allow(dead_code, reason = "this code may not be dead depending on features")]
#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    /// Deliberate user request to go to the next song.
    Next,

    /// Sent by the audio waiter whenever it believes a track has ended.
    End,

    /// When a track is loaded after the caller previously being told to wait.
    /// If a track is taken from the queue, then there is no waiting, so this
    /// is never actually sent.
    Loaded,

    /// Similar to Next, but specific to the first track.
    Init,

    /// Unpause the [Sink].
    Play,

    /// Pauses the [Sink].
    Pause,

    /// Pauses the [Sink]. This will also unpause it if it is paused.
    PlayPause,

    /// Change the volume of playback.
    ChangeVolume(f32),

    /// Set the volume of playback, rather than changing it.
    SetVolume(f32),

    /// Bookmark the current track.
    Bookmark,

    /// Quits gracefully.
    Quit,
}
