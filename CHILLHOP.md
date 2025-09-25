# Using the chillhop list

> [!WARNING]
> As of lowfi 1.7.0, the chillhop list is included by default. For a more
> detailed explanation, see [MUSIC.md](MUSIC.md). This document is included
> to preserve any old links or references. The instructions are still valid.

## Linux

```sh
mkdir -p ~/.local/share/lowfi
curl https://raw.githubusercontent.com/talwat/lowfi/refs/heads/main/data/chillhop.txt -O --output-dir ~/.local/share/lowfi
```

## MacOS

```sh
mkdir -p "$HOME/Library/Application Support/lowfi"
curl https://raw.githubusercontent.com/talwat/lowfi/refs/heads/main/data/chillhop.txt -O --output-dir "$HOME/Library/Application Support/lowfi"
```

## Windows

Go to `%appdata%` in Explorer, then `Roaming`, and make a folder called `lowfi`.
Then just put [this file](https://raw.githubusercontent.com/talwat/lowfi/refs/heads/main/data/chillhop.txt) in there.

## Launching lowfi

Once the list has been added, just launch `lowfi` with `-t chillhop`.
