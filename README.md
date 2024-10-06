# lowfi

lowfi is a tiny rust app that serves a single purpose: play lofi.
It'll do this as simply as it can: no albums, no ads, just lofi.

![example image](media/example1.png)

## Disclaimer

**All** of the audio files played in lowfi are from [Lofi Girl's](https://lofigirl.com/) website,
under their [licensing guidelines](https://form.lofigirl.com/CommercialLicense).

If god forbid you're planning to use this in a commercial setting, please
follow their rules.

## Why?

I really hate modern music platforms, and I wanted a small, "suckless"
app that would literally just play lofi without video so I could use it
whenever.

I also wanted it to be fairly resiliant to inconsistent networks,
so it buffers 5 whole songs at a time instead of parts of the same song.

Although, lowfi is yet to be properly tested in difficult conditions,
so don't rely on it too much until I do that. See [Scraping](#scraping) if
you're interested in downloading the tracks. Beware, there's a lot of them.

## Installing

You'll need Rust 1.74.0+.

If you're on linux, you also need the OpenSSL & Alsa headers.
Alsa can be installed with `alsa-lib` on Arch or `libasound2-dev` on Ubuntu.

### Cargo

The recommended installation method is to use cargo:

```sh
cargo install lowfi
```

and making sure `$HOME/.cargo/bin` is added to `$PATH`.

### AUR

If you're on Arch, you can also use the AUR:

```sh
yay -S lowfi
```

### Manual

```sh
git clone https://github.com/talwat/lowfi
cd lowfi

cargo build --release
./target/release/lowfi
```

## Usage

`lowfi`

Yeah, that's it. Controls are documented in the app.

### Extra Flags

If you're having issues on Linux with error messages,
or you don't want your terminal history to be visible when you
run lofi, you can use the `--alternate` or `-a` flag to
hide your history.

### Scraping

lowfi also has a `scrape` command which is usually not relevant, but
if you're trying to download some files from Lofi Girls' website,
it can be useful.

An example of scrape is as follows,

`lowfi scrape --extension zip --include-full`

where more information can be found by running `lowfi help scrape`.
