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
        Key::Escape if state.edit.is_some() => {
            state::cancel_edit(state);
            true
        }
        Key::Down => state::move_selection(state, 1, tx),
        Key::Up => state::move_selection(state, -1, tx),
        Key::Home => state::select_first(state, tx),
        Key::End => state::select_last(state, tx),
        Key::Text(text) => {
            let typing = matches!(
                tx.focused(),
                Some("search" | "unlock-password" | "edit-name")
            );
            if typing {
                return false;
            }
            match text.as_str() {
                "/" => {
                    tx.focus("search", true);
                    true
                }
                "j" => state::move_selection(state, 1, tx),
                "k" => state::move_selection(state, -1, tx),
                "h" => {
                    state::select_category(state, state.category.step(-1));
                    true
                }
                "l" => {
                    state::select_category(state, state.category.step(1));
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Approval, FieldValue, ItemField};
    use rune::{Env, Mods, Runtime};

    fn runtime(scene: impl FnOnce(&mut State)) -> Runtime<State> {
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
            "detail-field-created",
            "detail-field-updated",
            "copy-password",
            "copy-uri-0",
            "copy-user",
            "favorite-toggle",
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
    fn revealed_fields_show_values_and_a_hide_control() {
        let _guard = backend::REVEAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        backend::clear_revealed();
        backend::stash_revealed("ui-reveal-test", "shown-secret".to_owned());
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.revealed = Some("ui-reveal-test".to_owned());
            state.details.as_mut().unwrap().fields.push(ItemField {
                key: "ui-reveal-test".to_owned(),
                label: "Test secret".to_owned(),
                value: FieldValue::Masked,
            });
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.find("reveal-ui-reveal-test").is_some());
        let shown = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("field-value-Test secret"))
            .expect("revealed field text")
            .text
            .expect("revealed field value");
        assert_eq!(shown, "shown-secret");
        backend::clear_revealed();
    }

    #[test]
    fn totp_field_shows_code_and_countdown() {
        backend::stash_totp("1", "123456".to_owned());
        let mut rt = runtime(state::scene_populated);
        rt.frame(1.0 / 60.0);
        let shown = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("field-value-TOTP"))
            .expect("TOTP field text")
            .text
            .expect("TOTP field value");
        assert_eq!(shown, "123456 · 18s");
        backend::clear_totp("1");
    }

    #[test]
    fn uri_field_opens_website() {
        let mut rt = runtime(state::scene_populated);
        rt.frame(0.0);
        let (x, y) = rt.find("open-uri-0").expect("open website button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert_eq!(rt.take_url_requests(), ["https://github.com"]);
    }

    #[test]
    fn attachments_show_metadata_and_actions() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.details.as_mut().unwrap().fields.clear();
            state.requests = Some(tx.clone());
        });
        rt.frame(0.0);
        for key in [
            "attachment-field-att-1",
            "preview-attachment-att-1",
            "download-attachment-att-1",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let shown = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("field-value-Attachment"))
            .expect("attachment field text")
            .text
            .expect("attachment metadata");
        assert!(shown.contains("notes.txt"));
        assert!(shown.contains("1.2 KB"));

        let (x, y) = rt
            .find("download-attachment-att-1")
            .expect("download button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::Attachment {
                item_id,
                attachment_id,
                filename,
                preview,
            } if item_id == "1" && attachment_id == "att-1" && filename == "notes.txt" && !preview
        )));
    }

    #[test]
    fn edit_form_opens_saves_and_cancels() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.details.as_mut().unwrap().fields.clear();
            state.requests = Some(tx.clone());
        });
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        let (x, y) = rt.find("edit-item").expect("edit button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in ["edit-form", "edit-name", "save-edit", "cancel-edit"] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        rt.state
            .edit
            .as_mut()
            .unwrap()
            .name
            .replace_text("Work GitHub");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("save-edit").expect("save button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.edit.is_none());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::UpdateName { id, name } if id == "1" && name == "Work GitHub"
        )));

        let (x, y) = rt.find("edit-item").expect("edit button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        });
        rt.frame(0.0);
        assert!(rt.state.edit.is_none());
    }

    #[test]
    fn details_offer_delete_and_restore_actions() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.details.as_mut().unwrap().fields.clear();
            state.requests = Some(tx.clone());
        });
        rt.frame(0.0);
        let (x, y) = rt.find("delete-item").expect("delete button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::Delete { id, permanent } if id == "1" && !permanent
        )));

        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_trash(state);
            state.requests = Some(tx.clone());
        });
        rt.frame(0.0);
        assert!(rt.find("restore-item").is_some());
        let (x, y) = rt.find("delete-item").expect("permanent delete button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rx.try_iter().next().is_none());
        assert_eq!(
            rt.state.notice.as_deref(),
            Some("Click again to delete permanently")
        );
        let (x, y) = rt.find("delete-item").expect("confirm delete button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::Delete { id, permanent } if id == "trash-1" && permanent
        )));
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
    fn clicking_an_item_selects_it() {
        let mut rt = runtime(state::scene_populated);
        rt.frame(0.0);
        let (x, y) = rt.find("item-2").expect("second item");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert_eq!(rt.state.selected.as_deref(), Some("2"));
    }

    #[test]
    fn vim_keys_navigate_items_and_categories() {
        let mut rt = runtime(state::scene_populated);
        rt.on_key = Some(on_key);
        rt.frame(0.0);

        let press = |rt: &mut Runtime<State>, key: Key| {
            rt.key(KeyEvent {
                key,
                mods: Mods::default(),
            });
            rt.frame(0.0);
        };
        press(&mut rt, Key::Text("j".to_owned()));
        assert_eq!(rt.state.selected.as_deref(), Some("2"));
        press(&mut rt, Key::Text("k".to_owned()));
        assert_eq!(rt.state.selected.as_deref(), Some("1"));
        press(&mut rt, Key::Text("l".to_owned()));
        assert_eq!(rt.state.category, state::Category::Favorites);
        press(&mut rt, Key::Text("h".to_owned()));
        assert_eq!(rt.state.category, state::Category::All);
        press(&mut rt, Key::Text("/".to_owned()));
        assert!(rt.state.query.focused);
        press(&mut rt, Key::Text("j".to_owned()));
        assert_eq!(rt.state.query.text, "j");
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
