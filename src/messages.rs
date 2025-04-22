/// Handles communication between the frontend & audio player.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Messages {
    /// Notifies the audio server that it should update the track.
    Next,

    /// Special in that this isn't sent in a "client to server" sort of way,
    /// but rather is sent by a child of the server when a song has not only
    /// been requested but also downloaded aswell.
    NewSong,

    /// This signal is only sent if a track timed out. In that case,
    /// lowfi will try again and again to retrieve the track.
    TryAgain,

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
