
![lowfi logo](./docs/media/header.svg)

---

[[Version française](./docs/fr/README.md)]

lowfi is a tiny rust app that serves a single purpose: play lofi.
It'll do this as simply as it can: no albums, no ads, just lofi.

![example image](./docs/media/example1.png)

## Disclaimer

As of the 1.7.0 version of lowfi, **all** of the audio files embedded
by default are from [chillhop](https://chillhop.com/). Read
[MUSIC](./docs/MUSIC.md) for more information.

## Why?

I really hate modern music platforms, and I wanted a small, simple  
app that would just play random ambient music without video and other fluff.

Beyond that, it was also designed to be fairly resilient to inconsistent networks,
and as such it buffers 5 whole songs at a time instead of parts of the same song.

## Installing

> [!NOTE]
>
> If you're interested in maintaining a package for `lowfi`
> on package managers such as homebrew and the like, open an issue.

### Dependencies

You'll need Rust 1.83.0+.

On MacOS & Windows, no extra dependencies are needed.

On Linux, you'll also need openssl & alsa, as well as their headers.

- `alsa-lib` on Arch, `libasound2-dev` on Ubuntu, `alsa-lib-devel` on Fedora.
- `openssl` on Arch, `libssl-dev` on Ubuntu, `openssl-devel` on Fedora.

Make sure to also install `pulseaudio-alsa` if you're using PulseAudio.

### Cargo

The recommended installation method is to use cargo:

```sh
cargo install lowfi

# If you want MPRIS support.
cargo install lowfi --features mpris
```

and making sure `$HOME/.cargo/bin` is added to `$PATH`.
Also see [Extra Features](#extra-features) for extended functionality.

### Release Binaries

If you're struggling or unwilling to use cargo, you can just download the
precompiled binaries from the [latest release](https://github.com/talwat/lowfi/releases/latest).

### AUR

```sh
yay -S lowfi
```

### openSUSE

```sh
zypper install lowfi
```

### Debian

> [!NOTE]
> This uses an unofficial Debian repository maintained by [Dario Griffo](https://github.com/dariogriffo).

```sh
curl -sS https://debian.griffo.io/3B9335DF576D3D58059C6AA50B56A1A69762E9FF.asc | gpg --dearmor --yes -o /etc/apt/trusted.gpg.d/debian.griffo.io.gpg
echo "deb https://debian.griffo.io/apt $(lsb_release -sc 2>/dev/null) main" | sudo tee /etc/apt/sources.list.d/debian.griffo.io.list
sudo apt install -y lowfi
```

### Fedora (COPR)

> [!NOTE]
> This uses an unofficial COPR repository by [FurqanHun](https://github.com/FurqanHun).

```sh
sudo dnf copr enable furqanhun/lowfi
sudo dnf install lowfi
```

### Manual

This is good for debugging, especially in issues.

```sh
git clone https://github.com/talwat/lowfi
cd lowfi

# If you want an actual binary
cargo build --release --all-features
./target/release/lowfi

# If you just want to test
cargo run --all-features
```

## Usage

`lowfi`

Yeah, that's it.

### Controls

| Key                | Function        |
| ------------------ | --------------- |
| `s`, `n`, `l`      | Skip Song       |
| `p`, Space         | Play/Pause      |
| `+`, `=`, `k`, `↑` | Volume Up 10%   |
| `→`                | Volume Up 1%    |
| `-`, `_`, `j`, `↓` | Volume Down 10% |
| `←`                | Volume Down 1%  |
| `q`, CTRL+C        | Quit            |
| `b`                | Bookmark        |

> [!NOTE]
> Besides its regular controls, lowfi offers compatibility with Media Keys
> and [MPRIS](https://wiki.archlinux.org/title/MPRIS) (with tools like `playerctl`).
>
> MPRIS is currently an [optional feature](#extra-features) in cargo (enabled with `--features mpris`)
> due to it being only for Linux, as well as the fact that the main point of
> lowfi is it's unique & minimal interface.

### Bookmarks

Bookmarks are lowfi's extremely simple answer to "what about if I'd like to save a track."
You can bookmark/unbookmark tracks with `b`, and play them with `lowfi -t bookmarks`.

From a technical perspective, your bookmarks are no different to any other track list,
and as such are also stored in the same directory.

### Extra Flags

If you have something you'd like to tweak about lowfi, you use additional flags which
slightly tweak the UI or behavior of the menu. The flags can be viewed with `lowfi --help`.

| Flag                                | Function                                            |
| ----------------------------------- | --------------------------------------------------- |
| `-a`, `--alternate`                 | Use an alternate terminal screen                    |
| `-m`, `--minimalist`                | Hide the bottom control bar                         |
| `-b`, `--borderless`                | Exclude borders in UI                               |
| `-p`, `--paused`                    | Start lowfi paused                                  |
| `-f`, `--fps`                       | FPS of the UI [default: 12]                         |
| `--timeout`                         | Timeout in seconds for music downloads [default: 3] |
| `-d`, `--debug`                     | Include ALSA & other logs                           |
| `-w`, `--width <WIDTH>`             | Width of the player, from 0 to 32 [default: 3]      |
| `-t`, `--track-list <TRACK_LIST>`   | Use a [custom track list](#custom-track-lists)      |
| `-s`, `--buffer-size <BUFFER_SIZE>` | Internal song buffer size [default: 5]              |

If you need something even more specific, see [ENVIRONMENT_VARS](./docs/ENVIRONMENT_VARS.md).

### Extra Features

lowfi uses cargo/rust's "feature" system to make certain parts of the program optional,
like those which are only expected to be used by a handful of users.

#### `scrape` - Scraping

This feature provides the `scrape` command.
It's usually not very useful, but is included for transparency's sake.

More information can be found by running `lowfi help scrape`.

#### `mpris` - MPRIS

Enables MPRIS. It's not rocket science.

#### `extra-audio-formats` - Extra Audio Formats

This is only relevant to those using a custom track list, in which case
it allows for more formats than just MP3. Those are FLAC, Vorbis, and WAV.

These should be sufficient for some 99% of music files people might want to play.
If you are dealing with the 1% using another audio format which is in
[this list](https://github.com/pdeljanov/Symphonia?tab=readme-ov-file#codecs-decoders), open an issue.

### Custom Track Lists

> [!NOTE]
> Some nice users, especially [danielwerg](https://github.com/danielwerg),
> have already made alternative track lists located in the [data](https://github.com/talwat/lowfi/blob/main/data/)
> directory of this repo. You can use them with lowfi by using the `--track-list` flag.
>
> Feel free to contribute your own list with a PR.

lowfi also supports custom track lists, although the default one from chillhop
is embedded into the binary.

To use a custom list, use the `--track-list` flag. This can either be a path to some file,
or it could also be the name of a file (without the `.txt` extension) in the data
directory.

> [!NOTE]
> Data directories:
>
> - Linux - `~/.local/share/lowfi`
> - macOS - `~/Library/Application Support/lowfi`
> - Windows - `%appdata%\Roaming\lowfi`

For example, `lowfi --track-list minipop` would load `~/.local/share/lowfi/minipop.txt`.
Whereas if you did `lowfi --track-list ~/Music/minipop.txt` it would load from that
specified directory.

All tracks must be in the MP3 format, unless lowfi has been compiled with the
`extra-audio-formats` feature which includes support for some others.

#### The Format

In lists, the first line is what's known as the header, followed by the rest of the tracks.
Each track will be first appended to the header, and then use the combination to download
the track.

> [!NOTE]
> lowfi *will not* put a `/` between the base & track for added flexibility,
> so for most cases you should have a trailing `/` in your header.

The exception to this is if the track name begins with a protocol like `https://`,
in which case the base will not be prepended to it. If all of your tracks are like this,
then you can put `noheader` as the first line and not have a header at all.

For example, in this list:

```txt
https://lofigirl.com/wp-content/uploads/
2023/06/Foudroie-Finding-The-Edge-V2.mp3
2023/04/2-In-Front-Of-Me.mp3
https://file-examples.com/storage/fe85f7a43b689349d9c8f18/2017/11/file_example_MP3_1MG.mp3
```

lowfi would download these three URLs:

- `https://lofigirl.com/wp-content/uploads/2023/06/Foudroie-Finding-The-Edge-V2.mp3`
- `https://file-examples.com/storage/fe85f7a43b689349d9c8f18/2017/11/file_example_MP3_1MG.mp3`
- `https://lofigirl.com/wp-content/uploads/2023/04/2-In-Front-Of-Me.mp3`

Additionally, you may also specify a custom display name for the track which is indicated by a `!`.
For example, if you had an entry like this:

```txt
2023/04/2-In-Front-Of-Me.mp3!custom name
```

Then lowfi would download from the first section, and display the second as the track name.

`file://` can be used in front a track/header to make lowfi treat it as a local file.
This is useful if you want to use a local file as the base URL, for example:

```txt
file:///home/user/Music/
file.mp3
file:///home/user/Other Music/second-file.mp3
```

Further examples can be found in the [data](https://github.com/talwat/lowfi/tree/main/data) folder.
