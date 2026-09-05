//! Windows System Media Transport Controls (SMTC) via [`souvlaki`].
//!
//! Registers with the OS so hardware media keys and the Windows media overlay
//! work while another app is focused. Soft-fails on init errors so playback
//! still works without SMTC.

use std::sync::Arc;
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig,
};
use tokio::sync::{broadcast, mpsc};

use crate::player::Current;
use crate::ui::{self, Update};
use crate::Message;

/// Handle to SMTC; keeps the dummy HWND alive and syncs metadata/playback.
pub struct Server {
    controls: MediaControls,
    /// Must outlive `controls` on Windows.
    _dummy_window: windows::DummyWindow,
    /// Background message-pump thread shutdown signal.
    pump_shutdown: Option<std::sync::mpsc::Sender<()>>,
    sink: Arc<rodio::Player>,
    current: Current,
    list: String,
    receiver: broadcast::Receiver<Update>,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(tx) = self.pump_shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Server {
    /// Maps a souvlaki event onto the shared [`Message`] bus.
    fn map_event(event: MediaControlEvent) -> Option<Message> {
        match event {
            MediaControlEvent::Play => Some(Message::Play),
            MediaControlEvent::Pause => Some(Message::Pause),
            MediaControlEvent::Toggle => Some(Message::PlayPause),
            MediaControlEvent::Next => Some(Message::Next),
            MediaControlEvent::Stop => Some(Message::Pause),
            MediaControlEvent::Quit => Some(Message::Quit),
            MediaControlEvent::SetVolume(volume) => Some(Message::SetVolume(volume as f32)),
            // Previous / seek / raise / open-uri are unsupported, like MPRIS.
            _ => None,
        }
    }

    /// Creates SMTC controls, or `None` if registration fails (playback continues).
    pub fn try_new(
        state: ui::State,
        sender: mpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> Option<Self> {
        match Self::new(state, sender, receiver) {
            Ok(server) => Some(server),
            Err(err) => {
                eprintln!("warning: Windows media controls unavailable: {err}");
                None
            }
        }
    }

    fn new(
        state: ui::State,
        sender: mpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> Result<Self, String> {
        let dummy_window = windows::DummyWindow::new()?;
        let hwnd = Some(dummy_window.handle.0 as _);

        let config = PlatformConfig {
            dbus_name: "lowfi",
            display_name: "lowfi",
            hwnd,
        };

        let mut controls =
            MediaControls::new(config).map_err(|e| format!("MediaControls::new failed: {e:?}"))?;

        controls
            .attach(move |event| {
                if let Some(message) = Self::map_event(event) {
                    // Sync callback (COM / Win32); avoid blocking the UI runtime.
                    let _ = sender.try_send(message);
                }
            })
            .map_err(|e| format!("MediaControls::attach failed: {e:?}"))?;

        let (pump_shutdown, pump_rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("lowfi-smtc-pump".into())
            .spawn(move || {
                while pump_rx.try_recv().is_err() {
                    windows::pump_event_queue();
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
            .map_err(|e| format!("failed to spawn SMTC message pump: {e}"))?;

        let mut server = Self {
            controls,
            _dummy_window: dummy_window,
            pump_shutdown: Some(pump_shutdown),
            sink: state.sink,
            current: state.current,
            list: state.tracklist,
            receiver,
        };

        // Initial overlay state.
        let _ = server.sync_playback();
        let _ = server.sync_metadata();

        Ok(server)
    }

    /// Applies player messages to SMTC metadata / playback status.
    pub fn handle(&mut self, message: &Message) -> ui::Result<()> {
        while let Ok(update) = self.receiver.try_recv() {
            if let Update::Track(current) = update {
                self.current = current;
            }
        }

        match message {
            Message::ChangeVolume(_) | Message::SetVolume(_) => Ok(()),
            Message::Play | Message::Pause | Message::PlayPause => self.sync_playback(),
            Message::Init | Message::Loaded | Message::Next => {
                self.sync_metadata()?;
                self.sync_playback()
            }
            _ => Ok(()),
        }
    }

    fn playback_status(&self) -> MediaPlayback {
        if self.current.loading() {
            MediaPlayback::Stopped
        } else if self.sink.is_paused() {
            MediaPlayback::Paused { progress: None }
        } else {
            MediaPlayback::Playing { progress: None }
        }
    }

    fn sync_playback(&mut self) -> ui::Result<()> {
        self.controls
            .set_playback(self.playback_status())
            .map_err(|e| ui::Error::MediaControls(format!("{e:?}")))?;
        Ok(())
    }

    fn sync_metadata(&mut self) -> ui::Result<()> {
        match &self.current {
            Current::Loading(_) => {
                self.controls
                    .set_metadata(MediaMetadata {
                        title: Some("Loading..."),
                        album: Some(self.list.as_str()),
                        artist: Some("lowfi"),
                        ..Default::default()
                    })
                    .map_err(|e| ui::Error::MediaControls(format!("{e:?}")))?;
            }
            Current::Track(track) => {
                let title = track.display.as_str();
                let album = self.list.as_str();
                self.controls
                    .set_metadata(MediaMetadata {
                        title: Some(title),
                        album: Some(album),
                        artist: Some("lowfi"),
                        duration: track.duration,
                        ..Default::default()
                    })
                    .map_err(|e| ui::Error::MediaControls(format!("{e:?}")))?;
            }
        }
        Ok(())
    }
}

/// Minimal hidden window + Win32 message pump required by SMTC.
///
/// Adapted from the souvlaki `print_events` example.
mod windows {
    use std::io::Error;
    use std::mem;

    use ::windows::core::w;
    use ::windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetAncestor,
        IsDialogMessageW, PeekMessageW, RegisterClassExW, TranslateMessage, GA_ROOT, MSG,
        PM_REMOVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_QUIT, WNDCLASSEXW,
    };

    pub struct DummyWindow {
        pub handle: HWND,
    }

    impl DummyWindow {
        pub fn new() -> Result<DummyWindow, String> {
            let class_name = w!("lowfi-smtc");

            let handle = unsafe {
                let instance = GetModuleHandleW(None)
                    .map_err(|e| format!("Getting module handle failed: {e}"))?;

                let wnd_class = WNDCLASSEXW {
                    cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
                    hInstance: instance.into(),
                    lpszClassName: class_name,
                    lpfnWndProc: Some(Self::wnd_proc),
                    ..Default::default()
                };

                if RegisterClassExW(&wnd_class) == 0 {
                    // Class may already be registered from a previous run in-process.
                    let err = Error::last_os_error();
                    if err.raw_os_error() != Some(1410) {
                        // ERROR_CLASS_ALREADY_EXISTS
                        return Err(format!("Registering class failed: {err}"));
                    }
                }

                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class_name,
                    w!("lowfi"),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .map_err(|e| format!("Message-only window creation failed: {e}"))?
            };

            Ok(DummyWindow { handle })
        }

        extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }

    impl Drop for DummyWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.handle);
            }
        }
    }

    pub fn pump_event_queue() -> bool {
        unsafe {
            let mut msg = MSG::default();
            let mut has_message = PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool();
            while msg.message != WM_QUIT && has_message {
                if !IsDialogMessageW(GetAncestor(msg.hwnd, GA_ROOT), &msg).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                has_message = PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool();
            }
            msg.message == WM_QUIT
        }
    }
}