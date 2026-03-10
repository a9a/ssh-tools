# ssh-quick-connect

Simple, portable Rust CLI for quick SSH connections.

## Features

- lists machines when the app starts
- interactive terminal UI with arrow-key navigation
- live fuzzy filtering by `id`, `name`, or `host` while selecting
- launches local `ssh` with the right arguments
- configurable connection list in a TOML file
- auto-creates an example config on first run
- non-interactive mode: `--list` and `--connect <name_or_id_or_fragment>`

## Build And Run

```bash
cargo build --release
./target/release/ssh-quick-connect
```

```bash
./target/release/ssh-quick-connect --list
./target/release/ssh-quick-connect --connect prod-app
```

On first start, the program creates a config file and exits.

## Config Location

- if `SSH_QC_CONFIG` is set, that path is used
- Linux/macOS: `$XDG_CONFIG_HOME/ssh-quick-connect/config.toml` or `~/.config/ssh-quick-connect/config.toml`
- Windows: `%APPDATA%\ssh-quick-connect\config.toml` or `%USERPROFILE%\.ssh-quick-connect\config.toml`

## Config Format

```toml
[[connections]]
id = "dev"
name = "dev-server"
host = "192.168.1.10"
user = "dev"
port = 22
identity_file = "~/.ssh/id_ed25519"
options = []
```

`id` is optional but recommended for fast filtering and stable shortcuts.
`options` is optional; prefer strict SSH options in security-sensitive environments.

## Security Notes

- on Unix, config file is created with `0600` permissions
- the app rejects config files writable by group/others
- the app rejects config files not owned by current user (or root)

## Tests

```bash
cargo test
```
