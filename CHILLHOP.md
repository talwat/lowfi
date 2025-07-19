# Using the chillhop list

## Linux

```sh
mkdir -p ~/.local/share/lowfi
curl https://raw.githubusercontent.com/talwat/lowfi/refs/heads/main/data/chillhop.txt -O --output-dir ~/.local/share/lowfi
```

## MacOS

```sh
mkdir -p "~/Library/Application Support/lowfi"
curl https://raw.githubusercontent.com/talwat/lowfi/refs/heads/main/data/chillhop.txt -O --output-dir "~/Library/Application Support/lowfi"
```

## Windows

Go to `%appdata%` in Explorer, then `Roaming`, and make a folder called `lowfi`.
Then just put [this file](https://raw.githubusercontent.com/talwat/lowfi/refs/heads/main/data/chillhop.txt) in there.

## Launching lowfi

Once the list has been added, just launch `lowfi` with `-t chillhop`.