//! Kurama — a Rune desktop UI over the existing vault backend.
//!
//! Backend calls run on a worker thread and arrive via `poll`.

#![forbid(unsafe_code)]

mod backend;
mod icons;
mod state;
mod theme;
mod ui;

use rune::{AppSpec, Key, KeyEvent, Tx};
use state::State;
use std::process::ExitCode;

const HOST_COMMANDS: &[&str] = &[
    "auth",
    "attachment",
    "config",
    "copy",
    "daemon",
    "generate",
    "health",
    "help",
    "send",
    "ssh-agent",
    "ssh-key",
    "totp",
    "vault",
];

fn main() -> ExitCode {
    rune::set_app_id("dev.kurama.Kurama");

    let mut args = std::env::args_os();
    args.next();
    if let Some(first) = args.next() {
        let name = first.to_string_lossy();
        if HOST_COMMANDS.contains(&name.as_ref()) {
            return kurama_host::cli::run();
        }
    }

    rune::run(AppSpec {
        min_size: Some((900.0, 580.0)),
        scenes: &[
            ("populated", state::scene_populated),
            ("stress", state::scene_stress),
            ("locked", state::scene_locked),
            ("disconnected", state::scene_disconnected),
            ("trash", state::scene_trash),
        ],
        setup: Some(state::setup),
        poll: Some(state::poll),
        on_key: Some(on_key),
        ..AppSpec::new("Kurama", (1160.0, 760.0), State::default, ui::view)
    });
    ExitCode::SUCCESS
}

fn on_key(state: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if state.snapshot.status != "unlocked" {
        return false;
    }
    match &event.key {
        Key::Down => state::move_selection(state, 1, tx),
        Key::Up => state::move_selection(state, -1, tx),
        Key::Home => state::select_first(state, tx),
        Key::End => state::select_last(state, tx),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Approval;
    use rune::{Env, Mods, Runtime};

    fn runtime(scene: fn(&mut State)) -> Runtime<State> {
        let mut state = State::default();
        scene(&mut state);
        Runtime::new(
            state,
            ui::view,
            (1160.0, 760.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        )
    }

    #[test]
    fn vault_shell_has_search_sidebar_list_and_details() {
        let mut rt = runtime(state::scene_populated);
        rt.frame(1.0 / 60.0);
        for key in [
            "root",
            "search",
            "sidebar",
            "folder-Work",
            "trash-nav",
            "items",
            "item-1",
            "item-icon-login-1",
            "details",
            "detail-icon-login",
            "status-bar",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn detail_fields_show_type_fields_and_mask_secrets() {
        let mut rt = runtime(state::scene_populated);
        rt.frame(1.0 / 60.0);
        for key in [
            "detail-field-username",
            "detail-field-uri-0",
            "detail-field-password",
            "detail-field-totp",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let masked = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("field-value-Password"))
            .expect("masked field text")
            .text
            .expect("masked field value");
        assert_eq!(masked, "••••••••");
    }

    #[test]
    fn arrow_keys_move_item_selection() {
        let mut rt = runtime(state::scene_populated);
        rt.on_key = Some(on_key);
        rt.frame(0.0);

        rt.key(KeyEvent {
            key: Key::Down,
            mods: Mods::default(),
        });
        rt.frame(0.0);
        assert_eq!(rt.state.selected.as_deref(), Some("2"));

        rt.key(KeyEvent {
            key: Key::Up,
            mods: Mods::default(),
        });
        rt.frame(0.0);
        assert_eq!(rt.state.selected.as_deref(), Some("1"));
    }

    #[test]
    fn clickable_controls_show_hover_feedback() {
        let mut rt = runtime(state::scene_populated);
        rt.frame(0.0);
        let (x, y) = rt.find("trash-nav").expect("trash button");
        rt.pointer_move(x, y);
        rt.frame(1.0 / 60.0);
        let hover = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("trash-nav"))
            .expect("tooling element")
            .hover;
        assert!(hover > 0.0, "hover was {hover}");
    }

    #[test]
    fn locked_state_hides_the_vault() {
        let mut rt = runtime(state::scene_locked);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("placeholder-title").is_some());
        assert!(rt.find("unlock-password").is_some());
        assert!(rt.find("unlock-button").is_some());
        assert!(rt.find("quick-unlock-button").is_some());
        assert!(rt.find("item-1").is_none());
    }

    #[test]
    fn trash_scene_shows_deleted_items() {
        let mut rt = runtime(state::scene_trash);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("item-trash-1").is_some());
        assert!(rt.find("item-1").is_none());
    }

    #[test]
    fn large_vault_only_builds_visible_rows() {
        let mut rt = runtime(state::scene_stress);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("item-stress-0").is_some());
        assert!(rt.find("item-stress-394").is_none());
    }

    #[test]
    fn pending_approval_is_visible() {
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.approvals = vec![Approval {
                id: 9,
                title: "Sign a Git commit?".to_owned(),
                action: "SSH login as git@github.com".to_owned(),
                key_name: "Laptop key".to_owned(),
                client: "git (ssh)".to_owned(),
            }];
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.find("approvals-banner").is_some());
    }
}
