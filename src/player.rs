//! Responsible for playing & queueing audio.
//! This also has the code for the underlying
//! audio server which adds new tracks.

use std::{collections::VecDeque, ffi::CString, sync::Arc, time::Duration};

use arc_swap::ArcSwapOption;
use downloader::Downloader;
use libc::freopen;
use reqwest::Client;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use tokio::{
    select,
    sync::{
        mpsc::{Receiver, Sender},
        RwLock,
    },
    task,
};

use crate::{
    play::PersistentVolume,
    tracks::{self, list::List},
    Args,
};

pub mod downloader;
pub mod ui;

#[cfg(feature = "mpris")]
pub mod mpris;

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
    Play,

    /// Pauses the [Sink].
    Pause,

    /// Pauses the [Sink]. This will also unpause it if it is paused.
    PlayPause,

    /// Change the volume of playback.
    ChangeVolume(f32),

    /// Quits gracefully.
    Quit,
}

/// The time to wait in between errors.
const TIMEOUT: Duration = Duration::from_secs(5);

/// The amount of songs to buffer up.
const BUFFER_SIZE: usize = 5;

/// Main struct responsible for queuing up & playing tracks.
// TODO: Consider refactoring [Player] from being stored in an [Arc],
// TODO: so `Arc<Player>` into containing many smaller [Arc]s, being just
// TODO: `Player` as the type.
// TODO:
// TODO: This is conflicting, since then it'd clone ~10 smaller [Arc]s
// TODO: every single time, which could be even worse than having an
// TODO: [Arc] of an [Arc] in some cases (Like with [Sink] & [Client]).
pub struct Player {
    /// [rodio]'s [`Sink`] which can control playback.
    pub sink: Sink,

    /// The [`TrackInfo`] of the current track.
    /// This is [`None`] when lowfi is buffering/loading.
    current: ArcSwapOption<tracks::Info>,

    /// This is the MPRIS server, which is initialized later on in the
    /// user interface.
    #[cfg(feature = "mpris")]
    mpris: tokio::sync::OnceCell<mpris_server::Server<mpris::Player>>,

    /// The tracks, which is a [VecDeque] that holds
    /// *undecoded* [Track]s.
    tracks: RwLock<VecDeque<tracks::Track>>,

    /// The actual list of tracks to be played.
    list: List,

    /// The initial volume level.
    volume: PersistentVolume,

    /// The web client, which can contain a UserAgent & some
    /// settings that help lowfi work more effectively.
    client: Client,

    /// The [OutputStreamHandle], which also can control some
    /// playback, is for now unused and is here just to keep it
    /// alive so the playback can function properly.
    _handle: OutputStreamHandle,

    /// The [OutputStream], which is just here to keep the playback
    /// alive and functioning.
    _stream: OutputStream,
}

// SAFETY: This is necessary because [OutputStream] does not implement [Send],
// due to some limitation with Android's Audio API.
// I'm pretty sure nobody will use lowfi with android, so this is safe.
unsafe impl Send for Player {}

// SAFETY: See implementation for [Send].
unsafe impl Sync for Player {}

impl Player {
    /// This gets the output stream while also shutting up alsa with [libc].
    fn silent_get_output_stream() -> eyre::Result<(OutputStream, OutputStreamHandle)> {
        // Get the file descriptor to stderr from libc.
        extern "C" {
            static stderr: *mut libc::FILE;
        }

        // This is a bit of an ugly hack that basically just uses `libc` to redirect alsa's
        // output to `/dev/null` so that it wont be shoved down our throats.

        // The mode which to redirect terminal output with.
        let mode = CString::new("w")?.as_ptr();

        // First redirect to /dev/null, which basically silences alsa.
        let null = CString::new("/dev/null")?.as_ptr();
        // SAFETY: Simple enough to be impossible to fail. Hopefully.
        unsafe { freopen(null, mode, stderr) };

        // Make the OutputStream while stderr is still redirected to /dev/null.
        let (stream, handle) = OutputStream::try_default()?;

        // Redirect back to the current terminal, so that other output isn't silenced.
        let tty = CString::new("/dev/tty")?.as_ptr();
        // SAFETY: See the first call to `freopen`.
        unsafe { freopen(tty, mode, stderr) };

        Ok((stream, handle))
    }

    /// Just a shorthand for setting `current`.
    async fn set_current(&self, info: tracks::Info) -> eyre::Result<()> {
        self.current.store(Some(Arc::new(info)));

        Ok(())
    }

    /// A shorthand for checking if `self.current` is [Some].
    pub fn current_exists(&self) -> bool {
        self.current.load().is_some()
    }

    /// Sets the volume of the sink, and also clamps the value to avoid negative/over 100% values.
    pub fn set_volume(&self, volume: f32) {
        self.sink.set_volume(volume.clamp(0.0, 1.0))
    }

    /// Initializes the entire player, including audio devices & sink.
    ///
    /// This also will load the track list & persistent volume.
    pub async fn new(args: &Args) -> eyre::Result<Self> {
        // Load the volume file.
        let volume = PersistentVolume::load().await?;

        // Load the track list.
        let list = List::load(&args.tracks).await?;

        // We should only shut up alsa forcefully if we really have to.
        let (_stream, handle) = if cfg!(target_os = "linux") && !args.alternate && !args.debug {
            Self::silent_get_output_stream()?
        } else {
            OutputStream::try_default()?
        };

        let sink = Sink::try_new(&handle)?;
        if args.paused {
            sink.pause();
        }

        let client = Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(TIMEOUT)
            .build()?;

        let player = Self {
            tracks: RwLock::new(VecDeque::with_capacity(5)),
            current: ArcSwapOption::new(None),
            client,
            sink,
            volume,
            list,
            _handle: handle,
            _stream,

            #[cfg(feature = "mpris")]
            mpris: tokio::sync::OnceCell::new(),
        };

        Ok(player)
    }

    /// This will play the next track, as well as refilling the buffer in the background.
    ///
    /// This will also set `current` to the newly loaded song.
    pub async fn next(&self) -> eyre::Result<tracks::Decoded> {
        let track = match self.tracks.write().await.pop_front() {
            Some(x) => x,
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.
            None => {
                // Serves as an indicator that the queue is "loading".
                // We're doing it here so that we don't get the "loading" display
                // for only a frame in the other case that the buffer is not empty.
                self.current.store(None);

                self.list.random(&self.client).await?
            }
        };

        let decoded = track.decode()?;

        // Set the current track.
        self.set_current(decoded.info.clone()).await?;

        Ok(decoded)
    }

    /// This basically just calls [`Player::next`], and then appends the new track to the player.
    ///
    /// This also notifies the background thread to get to work, and will send `TryAgain`
    /// if it fails. This functions purpose is to be called in the background, so that
    /// when the audio server recieves a `Next` signal it will still be able to respond to other
    /// signals while it's loading.
    async fn handle_next(
        player: Arc<Self>,
        itx: Sender<()>,
        tx: Sender<Messages>,
    ) -> eyre::Result<()> {
        // Stop the sink.
        player.sink.stop();

        let track = player.next().await;

        match track {
            Ok(track) => {
                // Start playing the new track.
                player.sink.append(track.data);

                // Notify the background downloader that there's an empty spot
                // in the buffer.
                Downloader::notify(&itx).await?;

                // Notify the audio server that the next song has actually been downloaded.
                tx.send(Messages::NewSong).await?
            }
            Err(error) => {
                if !error.downcast::<reqwest::Error>()?.is_timeout() {
                    tokio::time::sleep(TIMEOUT).await;
                }

                tx.send(Messages::TryAgain).await?
            }
        };

        Ok(())
    }

    /// This is the main "audio server".
    ///
    /// `rx` & `tx` are used to communicate with it, for example when to
    /// skip tracks or pause.
    pub async fn play(
        player: Arc<Self>,
        tx: Sender<Messages>,
        mut rx: Receiver<Messages>,
    ) -> eyre::Result<()> {
        // `itx` is used to notify the `Downloader` when it needs to download new tracks.
        let downloader = Downloader::new(player.clone());
        let (itx, downloader) = downloader.start().await;

        // Start buffering tracks immediately.
        Downloader::notify(&itx).await?;

        // Set the initial sink volume to the one specified.
        player.set_volume(player.volume.float());

        // Whether the last signal was a `NewSong`.
        // This is helpful, since we only want to autoplay
        // if there hasn't been any manual intervention.
        let mut new = false;

        loop {
            let clone = Arc::clone(&player);

            let msg = select! {
                biased;

                Some(x) = rx.recv() => x,
                // This future will finish only at the end of the current track.
                // The condition is a kind-of hack which gets around the quirks
                // of `sleep_until_end`.
                //
                // That's because `sleep_until_end` will return instantly if the sink
                // is uninitialized. That's why we put a check to make sure that the last
                // signal we got was `NewSong`, since we shouldn't start waiting for the
                // song to be over until it has actually started.
                //
                // It's also important to note that the condition is only checked at the
                // beginning of the loop, not throughout.
                Ok(_) = task::spawn_blocking(move || clone.sink.sleep_until_end()),
                        if new => Messages::Next,
            };

            match msg {
                Messages::Next | Messages::Init | Messages::TryAgain => {
                    // We manually skipped, so we shouldn't actually wait for the song
                    // to be over until we recieve the `NewSong` signal.
                    new = false;

                    // This basically just prevents `Next` while a song is still currently loading.
                    if msg == Messages::Next && !player.current_exists() {
                        continue;
                    }

                    // Handle the rest of the signal in the background,
                    // as to not block the main audio thread.
                    task::spawn(Self::handle_next(player.clone(), itx.clone(), tx.clone()));
                }
                Messages::Play => {
                    player.sink.play();
                }
                Messages::Pause => {
                    player.sink.pause();
                }
                Messages::PlayPause => {
                    if player.sink.is_paused() {
                        player.sink.play();
                    } else {
                        player.sink.pause();
                    }
                }
                Messages::ChangeVolume(change) => {
                    player.set_volume(player.sink.volume() + change);
                }
                // This basically just continues, but more importantly, it'll re-evaluate
                // the select macro at the beginning of the loop.
                // See the top section to find out why this matters.
                Messages::NewSong => {
                    // We've recieved `NewSong`, so on the next loop iteration we'll
                    // begin waiting for the song to be over in order to autoplay.
                    new = true;

                    continue;
                }
                Messages::Quit => break,
            }
        }

        downloader.abort();

        Ok(())
    }
}
