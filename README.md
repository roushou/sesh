# sesh

A fast, minimal SSH connection manager for VPS hosts. Store, tag, and connect to your servers from the command line.

## Installation

### From crates.io

```sh
cargo install sesh-ssh
```

### From source

Requires the Rust nightly toolchain.

```sh
git clone https://github.com/roushou/sesh.git
cd sesh
cargo install --path .
```

### From GitHub releases

Download a prebuilt binary from the [releases page](https://github.com/roushou/sesh/releases) and place it somewhere on your `PATH`.

## Quick start

```sh
# Import hosts from your SSH config
sesh import

# Connect (saves then connects)
sesh connect web-1 --host 203.0.113.10 --user root --port 2222 --tags prod,web --provider hetzner

# Connect to known host
sesh connect web-1

# List all hosts
sesh list
```

## Commands

### `sesh add <name>`

Add a host entry manually.

| Flag               | Description                          | Default |
| ------------------ | ------------------------------------ | ------- |
| `--host`           | Hostname or IP (required)            |         |
| `--user`           | SSH user                             |         |
| `--port`           | SSH port                             | `22`    |
| `--identity-file`  | Path to private key (`-i`)           |         |
| `--tags`           | Comma-separated tags                 |         |
| `--provider`       | Provider label (e.g. `hetzner`)      |         |

### `sesh import`

Import host entries from an SSH config file.

| Flag            | Description                                   | Default           |
| --------------- | --------------------------------------------- | ----------------- |
| `--file`        | Path to SSH config                            | `~/.ssh/config`   |
| `--overwrite`   | Replace existing entries with imported values  |                   |
| `--provider`    | Provider label for all imported entries        |                   |
| `--tags`        | Comma-separated tags for all imported entries  |                   |

### `sesh list`

List stored host entries in a table.

| Flag          | Description          |
| ------------- | -------------------- |
| `--tag`       | Filter by tag        |
| `--provider`  | Filter by provider   |

### `sesh connect <target>`

Connect using system `ssh`.

- If `<target>` matches a stored entry name, it connects directly.
- If `<target>` does not exist, you can auto-add and save in one command:
  - `sesh connect <name> --host <hostname>`
  - `sesh connect user@host` (saved under `host` unless `--save-as` is set)

| Flag            | Description                                                     | Default |
| --------------- | --------------------------------------------------------------- | ------- |
| `--host`        | Hostname/IP for one-command auto-add                            |         |
| `--user`        | SSH user for auto-add                                           |         |
| `--port`        | SSH port for auto-add                                           | `22`    |
| `--identity-file`| Private key path for auto-add (`-i`)                          |         |
| `--tags`        | Comma-separated tags for auto-add                               |         |
| `--provider`    | Provider label for auto-add                                     |         |
| `--save-as`     | Entry name to save auto-added destination under                 |         |
| `--dry-run`     | Print the SSH command without running                           |         |

### `sesh doctor [name]`

Run environment and host health checks. Verifies that `ssh` is available, the store file exists, and that hosts are reachable over TCP with a valid SSH banner.

| Flag             | Description                     | Default  |
| ---------------- | ------------------------------- | -------- |
| `--timeout-ms`   | TCP/banner timeout in ms        | `1500`   |

## Storage

Host entries are stored in `~/.config/sesh/hosts.toml`.

```toml
version = 1

[[hosts]]
name = "web-1"
host = "203.0.113.10"
user = "root"
port = 2222
identity_file = "/home/me/.ssh/id_ed25519"
tags = ["prod", "web"]
provider = "hetzner"
```

## License

MIT
