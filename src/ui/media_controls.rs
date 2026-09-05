//! Windows System Media Transport Controls (SMTC) via [`souvlaki`].
//! Soft-fails on init so playback still works without SMTC.

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
    pump_shutdown: std::sync::mpsc::Sender<()>,
    sink: Arc<rodio::Player>,
    current: Current,
    list: String,
    receiver: broadcast::Receiver<Update>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.pump_shutdown.send(());
    }
}

impl Server {
    fn map_event(event: MediaControlEvent) -> Option<Message> {
        match event {
            MediaControlEvent::Play => Some(Message::Play),
            MediaControlEvent::Pause | MediaControlEvent::Stop => Some(Message::Pause),
            MediaControlEvent::Toggle => Some(Message::PlayPause),
            MediaControlEvent::Next => Some(Message::Next),
            MediaControlEvent::Quit => Some(Message::Quit),
            MediaControlEvent::SetVolume(v) => Some(Message::SetVolume(v as f32)),
            _ => None,
        }
    }

    /// Creates SMTC controls, or `None` if registration fails (playback continues).
    pub fn try_new(
        state: ui::State,
        sender: mpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> Option<Self> {
        match Self::create(state, sender, receiver) {
            Ok(server) => Some(server),
            Err(err) => {
                eprintln!("warning: Windows media controls unavailable: {err}");
                None
            }
        }
    }

    fn create(
        state: ui::State,
        sender: mpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> Result<Self, String> {
        let dummy_window = windows::DummyWindow::new()?;
        let config = PlatformConfig {
            dbus_name: "lowfi",
            display_name: "lowfi",
            hwnd: Some(dummy_window.handle.0 as _),
        };

        let mut controls =
            MediaControls::new(config).map_err(|e| format!("MediaControls::new: {e:?}"))?;
        controls
            .attach(move |event| {
                if let Some(msg) = Self::map_event(event) {
                    let _ = sender.try_send(msg);
                }
            })
            .map_err(|e| format!("MediaControls::attach: {e:?}"))?;

        let (pump_shutdown, pump_rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("lowfi-smtc-pump".into())
            .spawn(move || {
                while pump_rx.try_recv().is_err() {
                    windows::pump_events();
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
            .map_err(|e| format!("SMTC pump spawn: {e}"))?;

        let mut server = Self {
            controls,
            _dummy_window: dummy_window,
            pump_shutdown,
            sink: state.sink,
            current: state.current,
            list: state.tracklist,
            receiver,
        };
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
            Message::Play | Message::Pause | Message::PlayPause => self.sync_playback(),
            Message::Init | Message::Loaded | Message::Next => {
                self.sync_metadata()?;
                self.sync_playback()
            }
            _ => Ok(()),
        }
    }

    fn sync_playback(&mut self) -> ui::Result<()> {
        let status = if self.current.loading() {
            MediaPlayback::Stopped
        } else if self.sink.is_paused() {
            MediaPlayback::Paused { progress: None }
        } else {
            MediaPlayback::Playing { progress: None }
        };
        self.controls
            .set_playback(status)
            .map_err(|e| ui::Error::MediaControls(format!("{e:?}")))
    }

    fn sync_metadata(&mut self) -> ui::Result<()> {
        let (title, duration) = match &self.current {
            Current::Loading(_) => ("Loading...", None),
            Current::Track(track) => (track.display.as_str(), track.duration),
        };
        self.controls
            .set_metadata(MediaMetadata {
                title: Some(title),
                album: Some(self.list.as_str()),
                artist: Some("lowfi"),
                duration,
                ..Default::default()
            })
            .map_err(|e| ui::Error::MediaControls(format!("{e:?}")))
    }
}

/// Minimal hidden window + Win32 message pump required by SMTC.
mod windows {
    use std::io::Error;
    use std::mem;

    use ::windows::core::w;
    use ::windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
        RegisterClassExW, TranslateMessage, MSG, PM_REMOVE, WINDOW_EX_STYLE, WINDOW_STYLE,
        WM_QUIT, WNDCLASSEXW,
    };

    pub struct DummyWindow {
        pub handle: HWND,
    }

    impl DummyWindow {
        pub fn new() -> Result<Self, String> {
            let class = w!("lowfi-smtc");
            unsafe {
                let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
                let wnd = WNDCLASSEXW {
                    cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
                    hInstance: instance.into(),
                    lpszClassName: class,
                    lpfnWndProc: Some(Self::wnd_proc),
                    ..Default::default()
                };
                if RegisterClassExW(&wnd) == 0 {
                    let err = Error::last_os_error();
                    // ERROR_CLASS_ALREADY_EXISTS
                    if err.raw_os_error() != Some(1410) {
                        return Err(err.to_string());
                    }
                }

                let handle = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class,
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
                .map_err(|e| e.to_string())?;

                Ok(Self { handle })
            }
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

    pub fn pump_events() {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}