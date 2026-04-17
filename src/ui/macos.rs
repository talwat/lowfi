//! Contains the code for macOS media controls via the Now Playing framework.

use std::{
    sync::{
        mpsc::{self, SyncSender},
        Arc,
    },
    time::Duration,
};

use core_foundation_sys::runloop::{kCFRunLoopDefaultMode, CFRunLoopRunInMode};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use tokio::sync::{broadcast, mpsc as tmpsc};

use crate::{player::Current, ui::Update, Message};

use super::State;

/// Internal update sent from the tokio runtime to the macOS media thread.
enum MacosUpdate {
    /// Update the Now Playing metadata with a new track title and duration.
    Metadata {
        /// The display name of the track.
        title: String,
        /// The duration of the track, if known.
        duration: Option<Duration>,
    },
    /// Update the playback status (playing, paused, or stopped).
    Playback(MediaPlayback),
    /// Shut down the media controls thread.
    Quit,
}

/// Handle to the macOS Now Playing / media controls background thread.
pub struct Server {
    /// Shared audio sink, used to read paused state.
    sink: Arc<rodio::Player>,

    /// The latest known track state.
    current: Current,

    /// Channel to send updates to the background thread.
    update_tx: SyncSender<MacosUpdate>,

    /// Broadcast receiver for track/state updates from the player.
    receiver: broadcast::Receiver<Update>,
}

impl Server {
    /// Creates the macOS media controls server and spawns the background thread.
    pub fn new(
        state: State,
        sender: tmpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> Self {
        let (update_tx, update_rx) = mpsc::sync_channel::<MacosUpdate>(8);

        std::thread::spawn(move || {
            let config = PlatformConfig {
                display_name: "lowfi",
                dbus_name: "dev.talwat.lowfi",
                hwnd: None,
            };

            let Ok(mut controls) = MediaControls::new(config) else {
                return;
            };

            let _ = controls.attach(move |event: MediaControlEvent| {
                let message = match event {
                    MediaControlEvent::Play => Message::Play,
                    MediaControlEvent::Pause => Message::Pause,
                    MediaControlEvent::Toggle => Message::PlayPause,
                    MediaControlEvent::Next => Message::Next,
                    MediaControlEvent::Quit => Message::Quit,
                    _ => return,
                };
                let _ = sender.try_send(message);
            });

            loop {
                // Drain all pending updates from the tokio side without blocking.
                loop {
                    match update_rx.try_recv() {
                        Ok(MacosUpdate::Metadata { title, duration }) => {
                            let _ = controls.set_metadata(MediaMetadata {
                                title: Some(title.as_str()),
                                album: None,
                                artist: None,
                                cover_url: None,
                                duration,
                            });
                        }
                        Ok(MacosUpdate::Playback(status)) => {
                            let _ = controls.set_playback(status);
                        }
                        Ok(MacosUpdate::Quit) | Err(mpsc::TryRecvError::Disconnected) => return,
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }

                // Pump this thread's CFRunLoop briefly, in case souvlaki dispatches
                // callbacks here rather than on the main queue.
                unsafe {
                    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, 0);
                }
            }
        });

        // Pump the main CFRunLoop on a ~60 Hz interval so that MPRemoteCommandCenter
        // callbacks (dispatched on GCD's main queue) are delivered. Tokio uses kqueue
        // internally and never spins the Cocoa run loop itself. With current_thread
        // flavor this task runs on the main thread, which is exactly where the main
        // queue needs to be drained.
        tokio::spawn(async {
            let mut interval = tokio::time::interval(Duration::from_millis(16));
            loop {
                interval.tick().await;
                unsafe {
                    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
                }
            }
        });

        Self {
            sink: state.sink,
            current: state.current,
            update_tx,
            receiver,
        }
    }

    /// Sends the current track metadata to the Now Playing widget.
    fn update_metadata(&self) {
        let Current::Track(track) = &self.current else {
            return;
        };

        let _ = self.update_tx.send(MacosUpdate::Metadata {
            title: track.display.clone(),
            duration: track.duration,
        });
    }

    /// Sends the current playback status to the Now Playing widget.
    fn update_playback(&self) {
        let status = if self.current.loading() {
            MediaPlayback::Stopped
        } else if self.sink.is_paused() {
            MediaPlayback::Paused { progress: None }
        } else {
            MediaPlayback::Playing { progress: None }
        };

        let _ = self.update_tx.send(MacosUpdate::Playback(status));
    }

    /// Handles a player message, keeping macOS media controls in sync.
    #[allow(clippy::unused_async)]
    pub async fn handle(&mut self, message: &Message) -> super::Result<()> {
        while let Ok(update) = self.receiver.try_recv() {
            if let Update::Track(current) = update {
                self.current = current;
            }
        }

        match message {
            Message::Init | Message::Loaded | Message::Next => {
                self.update_metadata();
                self.update_playback();
            }
            Message::Play | Message::Pause | Message::PlayPause => {
                self.update_playback();
            }
            Message::Quit => {
                let _ = self.update_tx.send(MacosUpdate::Quit);
            }
            _ => {}
        }

        Ok(())
    }
}
