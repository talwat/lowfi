/// Handles communication between different parts of the program.
#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    /// Notifies the audio server that it should update the track.
    Next,

    /// When a track is loaded after a caller previously being told to wait.
    Loaded,

    /// Similar to Next, but specific to the first track.
    Init,

    /// Unpause the [Sink].
    #[allow(dead_code, reason = "this code may not be dead depending on features")]
    Play,

    /// Pauses the [Sink].
    Pause,

    /// Pauses the [Sink]. This will also unpause it if it is paused.
    PlayPause,

    /// Change the volume of playback.
    ChangeVolume(f32),

    /// Bookmark the current track.
    Bookmark,

    /// Quits gracefully.
    Quit,
}
