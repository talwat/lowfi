//! Contains the code for macOS media controls via the Now Playing framework.

use std::{sync::Arc, time::Duration};

use core_foundation_sys::runloop::{kCFRunLoopDefaultMode, CFRunLoopRunInMode};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use tokio::sync::{broadcast, mpsc as tmpsc};

use crate::{player::Current, ui::Update, Message};

use super::State;

/// Handle to the macOS Now Playing media controls.
pub struct Server {
    /// Shared audio sink, used to read paused state.
    sink: Arc<rodio::Player>,

    /// The latest known track state.
    current: Current,

    /// The souvlaki media controls handle. `None` if initialisation failed.
    controls: Option<MediaControls>,

    /// Broadcast receiver for track/state updates from the player.
    receiver: broadcast::Receiver<Update>,
}

impl Server {
    /// Creates the macOS media controls server.
    pub fn new(
        state: State,
        sender: tmpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> Self {
        let config = PlatformConfig {
            display_name: "lowfi",
            dbus_name: "dev.talwat.lowfi",
            hwnd: None,
        };

        let controls = MediaControls::new(config).ok().and_then(|mut controls| {
            controls
                .attach(move |event: MediaControlEvent| {
                    let message = match event {
                        MediaControlEvent::Play => Message::Play,
                        MediaControlEvent::Pause => Message::Pause,
                        MediaControlEvent::Toggle => Message::PlayPause,
                        MediaControlEvent::Next => Message::Next,
                        MediaControlEvent::Quit => Message::Quit,
                        _ => return,
                    };
                    let _ = sender.try_send(message);
                })
                .ok()?;
            Some(controls)
        });

        // Pump the main CFRunLoop on a ~60 Hz interval so that MPRemoteCommandCenter
        // callbacks (dispatched on GCD's main queue) are delivered. Tokio uses kqueue
        // internally and never spins the Cocoa run loop itself. With current_thread
        // flavor this task runs on the main thread, which is exactly where the main
        // queue needs to be drained.
        tokio::spawn(async {
            let mut interval = tokio::time::interval(Duration::from_millis(16));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
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
            controls,
            receiver,
        }
    }

    /// Sends the current track metadata to the Now Playing widget.
    fn update_metadata(&mut self) {
        let Current::Track(track) = &self.current else {
            return;
        };

        let Some(controls) = self.controls.as_mut() else {
            return;
        };

        let _ = controls.set_metadata(MediaMetadata {
            title: Some(track.display.as_str()),
            album: None,
            artist: None,
            cover_url: None,
            duration: track.duration,
        });
    }

    /// Sends the current playback status to the Now Playing widget.
    fn update_playback(&mut self) {
        let Some(controls) = self.controls.as_mut() else {
            return;
        };

        let status = if self.current.loading() {
            MediaPlayback::Stopped
        } else if self.sink.is_paused() {
            MediaPlayback::Paused { progress: None }
        } else {
            MediaPlayback::Playing { progress: None }
        };

        let _ = controls.set_playback(status);
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
                if let Some(controls) = self.controls.as_mut() {
                    let _ = controls.set_playback(MediaPlayback::Stopped);
                }
            }
            _ => {}
        }

        Ok(())
    }
}
