# Kurama

Kurama is a native desktop vault application built with [Rune](https://github.com/anomalyco/rune). It reuses the proven `omawarden` backend — Bitwarden/Vaultwarden API client, encryption, local cache, daemon and SSH agent — and replaces the original Omarchy QML overlay with a Rune UI.

This is an early foundation: the app shell, theme tokens, category rail, item list, detail pane and status bar are in place, and item data is read from the backend daemon on a worker thread. Authentication prompts, item editing and the extended feature panels are still to come.

## Credits

Kurama is based on **[omarchy-bitwarden](https://github.com/icyleaf/omarchy-bitwarden)** by **icyleaf**, licensed under the MIT License. The `omawarden/` directory in this repository is that project's Rust backend, retained so existing vault data, credentials and behaviour keep working. Thank you for making the engine this UI builds on.

The UI is built with [Rune](https://github.com/anomalyco/rune), whose `examples/hello` app provided the starting structure (state, view, `AppSpec`).

## Layout

- `kurama/` — Rune application (UI, state, backend bridge).
- `omawarden/` — original backend crate and CLI, used as a library and daemon.
- `rune/` — pinned Rune source tree used as a local path dependency.

## Running

Start the backend daemon first, then the UI:

```sh
cargo build --release --manifest-path omawarden/Cargo.toml   # provides `omawarden`
omawarden daemon &
cargo run -p kurama
```

Kurama connects over the same user-owned Unix socket used by `omawarden`. If the daemon is not running, the app shows a disconnected state with a Retry button; it never starts or reimplements backend logic itself. Unlock and login still use the existing backend commands (`omawarden auth unlock`, `omawarden auth login-password`, etc.).

`cargo run -p kurama -- --snap kurama.png --scene populated` renders a headless snapshot; `--scene locked` and `--scene disconnected` show the other states.

## Integration notes

- All daemon requests are made on a worker thread with `omawarden::daemon::send_daemon_request`; `poll` applies results on the UI thread, so rendering never blocks on IPC.
- Only non-secret item metadata crosses into UI state. Passwords, notes and decrypted keys stay inside the daemon.
- The root workspace patches Rune's vendored `winit`, `wgpu-hal`, `sctk-adwaita`, `parley_engine` and AccessKit crates, as an external Rune app must.
