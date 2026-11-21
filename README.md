# Kurama

Kurama is a native desktop vault application built with [Rune](https://github.com/anomalyco/rune). It uses the `kurama-host` backend — Bitwarden/Vaultwarden API client, encryption, local cache, daemon and SSH agent — in place of the original Omarchy QML overlay.

The UI now covers authentication and unlock, vault browsing and editing, favorites and folders, TOTP, password generation, Sends, attachments, SSH tools, settings and keyboard shortcuts. Item data is read from the backend daemon on a worker thread, and only non-secret metadata is kept in UI state.

## Credits

Kurama is based on **[omarchy-bitwarden](https://github.com/icyleaf/omarchy-bitwarden)** by **icyleaf**, licensed under the MIT License. Its Rust backend was renamed to `kurama-host`; the original behaviour is preserved so existing vault data and credentials keep working. Thank you for making the engine this UI builds on.

The UI is built with [Rune](https://github.com/anomalyco/rune), whose `examples/hello` app provided the starting structure (state, view, `AppSpec`).

## Layout

- `kurama/` — Rune application (UI, state, backend bridge).
- `kurama-host/` — backend library and daemon, renamed from `omawarden`.
- `rune/` — pinned Rune source tree used as a local path dependency.

## Setup

The Rune source URL and revision are pinned in `rune-pin.toml`. Run `./scripts/setup-rune.sh` on a fresh checkout; the build verifies `rune/` is at that revision and fails clearly on a mismatch.

## Running

Start the `kurama-host` daemon first, then the UI:

```sh
cargo build --release -p kurama   # provides ./target/release/kurama
./target/release/kurama daemon &
./target/release/kurama
```

Kurama connects over a user-owned Unix socket. If the daemon is not running, the app shows a disconnected state with a Retry button; it never starts or reimplements backend logic itself. The same binary provides all backend commands: `kurama auth unlock`, `kurama auth login-password`, etc.

Use a debug build for headless snapshots (`cargo run -p kurama -- --snap /tmp/kurama.png --scene populated`); release builds omit Rune's `tooling` feature. `--scene locked` and `--scene disconnected` show the other states.

## Integration notes

- All daemon requests are made on a worker thread with `kurama_host::daemon::send_daemon_request`; `poll` applies results on the UI thread, so rendering never blocks on IPC.
- On first run, legacy `~/.config/omawarden/config.json`, `~/.local/share/omawarden/data.json` and `omarchy-bitwarden` keyring entries are migrated or read through fallbacks. Old `OMAWARDEN_*` environment variables remain accepted. The legacy keyring entries are retained as a fallback and are cleared on logout.
- Only non-secret item metadata crosses into UI state. Passwords, notes and decrypted keys stay inside the daemon.
- The daemon exposes an `events` action; Kurama polls it for status, sync, error and SSH approval notices instead of registering shell callbacks.
- The root workspace patches Rune's vendored `winit`, `wgpu-hal`, `sctk-adwaita`, `parley_engine` and AccessKit crates, as an external Rune app must.
