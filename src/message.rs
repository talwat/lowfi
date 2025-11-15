use crate::ui;

/// Handles communication between different parts of the program.
#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    /// Sent to update the UI with new information.
    Render(ui::Update),

    /// Notifies the audio server that it should update the track.
    Next,

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
