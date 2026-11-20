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
            ("account", state::scene_account),
            ("account-two-factor", state::scene_account_two_factor),
            ("account-fido2", state::scene_account_fido2),
            ("session-expired", state::scene_session_expired),
            ("trash", state::scene_trash),
            ("create-card", state::scene_create_card),
            ("create-identity", state::scene_create_identity),
            ("create-ssh", state::scene_create_ssh),
            ("create-fields", state::scene_create_fields),
            ("create-uris", state::scene_create_uris),
            ("generator", state::scene_generator),
            ("settings", state::scene_settings),
            ("sends", state::scene_sends),
            ("send-form", state::scene_send_form),
            ("attachment-preview", state::scene_attachment_preview),
            ("ssh-tools", state::scene_ssh_tools),
            ("ssh-approval", state::scene_ssh_approval),
            ("sync-error", state::scene_sync_error),
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
    if event.is_command('l') {
        state::dispatch(state, backend::Request::Lock);
        return true;
    }
    if event.is_command('r') {
        state::dispatch(state, backend::Request::Sync);
        return true;
    }
    match &event.key {
        Key::Escape if state.settings_open => {
            state::close_settings(state);
            true
        }
        _ if state.settings_open => false,
        Key::Escape if state.help => {
            state.help = false;
            true
        }
        Key::Escape if state.attachment_preview.is_some() => {
            state::close_attachment(state);
            true
        }
        Key::Escape if state.ssh_tools.is_some() => {
            state::close_ssh_tools(state);
            true
        }
        Key::Escape if state.edit.is_some() => {
            state::cancel_edit(state);
            true
        }
        Key::Escape if state.create.is_some() => {
            state::cancel_create(state);
            true
        }
        Key::Escape if state.send_form.is_some() => {
            state::cancel_send(state);
            true
        }
        Key::Escape if state.send_pending_delete.is_some() => {
            state::cancel_delete_send(state);
            true
        }
        Key::Escape if state.sends_open => {
            state::close_sends(state);
            true
        }
        Key::Escape if state.generator.is_some() => {
            state::close_generator(state);
            true
        }
        Key::Down => state::move_selection(state, 1, tx),
        Key::Up => state::move_selection(state, -1, tx),
        Key::Home => state::select_first(state, tx),
        Key::End => state::select_last(state, tx),
        Key::Text(text) => {
            let focused = tx.focused();
            let typing = matches!(
                focused,
                Some(
                    "search"
                        | "unlock-password"
                        | "edit-name"
                        | "edit-username"
                        | "edit-password"
                        | "create-name"
                        | "create-username"
                        | "create-password"
                        | "create-notes"
                        | "edit-notes"
                        | "create-first-name"
                        | "create-last-name"
                        | "create-email"
                        | "create-phone"
                        | "create-address1"
                        | "create-postal-code"
                        | "create-city"
                        | "create-country"
                        | "edit-first-name"
                        | "edit-last-name"
                        | "edit-email"
                        | "edit-phone"
                        | "edit-address1"
                        | "edit-postal-code"
                        | "edit-city"
                        | "edit-country"
                        | "create-private-key"
                        | "create-public-key"
                        | "create-fingerprint"
                        | "generator-separator"
                        | "generator-value"
                        | "send-name"
                        | "send-text"
                        | "send-max-access"
                        | "send-expiration"
                        | "send-password"
                )
            ) || focused.is_some_and(|key| {
                key.starts_with("create-card")
                    || key.starts_with("edit-card")
                    || key.starts_with("create-field-name-")
                    || key.starts_with("create-field-value-")
                    || key.starts_with("create-uri-")
                    || key.starts_with("edit-uri-")
                    || key.starts_with("ssh-tool-")
                    || key.starts_with("account-")
            });
            if typing {
                return false;
            }
            match text.as_str() {
                "?" => {
                    state.help = !state.help;
                    true
                }
                "/" => {
                    tx.focus("search", true);
                    true
                }
                "n" => {
                    state.create = Some(state::start_create());
                    state.form_error = None;
                    tx.focus("create-name", true);
                    true
                }
                "e" => {
                    if state.selected_item().is_none() {
                        return false;
                    }
                    state::start_edit(state);
                    if state.edit.is_some() {
                        tx.focus("edit-name", true);
                    }
                    true
                }
                "c" => {
                    state::copy_field(state, "password", "Password", true);
                    true
                }
                "," => {
                    state::open_settings(state);
                    true
                }
                "g" => {
                    state::open_generator(state, state::GeneratorTarget::None);
                    true
                }
                "s" => {
                    state::open_sends(state);
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
    use crate::backend::{Approval, FieldValue, ItemField, Request};
    use crate::theme::ThemePreference;
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
    fn field_labels_share_input_padding() {
        let mut rt = runtime(state::scene_ssh_tools);
        rt.frame(1.0 / 60.0);
        let elements = rt.elements();
        for (label, input) in [
            ("ssh-tool-name-label", "ssh-tool-name"),
            ("ssh-tool-comment-label", "ssh-tool-comment"),
        ] {
            let label_x = elements
                .iter()
                .find(|element| element.key.as_deref() == Some(label))
                .unwrap_or_else(|| panic!("missing label: {label}"))
                .rect[0];
            let input_x = elements
                .iter()
                .find(|element| element.key.as_deref() == Some(input))
                .unwrap_or_else(|| panic!("missing input: {input}"))
                .rect[0];
            let expected_input_x = label_x + 12.0;
            assert!(
                (expected_input_x - input_x).abs() < 0.01,
                "{label} at {label_x}, {input} at {input_x}"
            );
        }
    }

    #[test]
    fn generator_scene_shows_password_and_result_controls() {
        let mut rt = runtime(state::scene_generator);
        rt.frame(1.0 / 60.0);
        for key in [
            "generator-form",
            "generator-modes",
            "generator-length",
            "generator-options",
            "generator-generate",
            "generator-value",
            "generator-entropy",
            "generator-copy",
            "generator-close",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn sync_error_scene_shows_failure_in_status_bar() {
        let mut rt = runtime(state::scene_sync_error);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("status-sync").is_some());
        let status = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("status-label"))
            .expect("status label")
            .text
            .expect("status text");
        assert!(status.contains("Sync failed"));
    }

    #[test]
    fn ssh_approval_scene_shows_agent_status_and_reply_actions() {
        let mut rt = runtime(state::scene_ssh_approval);
        rt.frame(1.0 / 60.0);
        for key in ["approvals-banner", "ssh-approve-42", "ssh-deny-42"] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        assert!(rt.state.agent_running);
    }

    #[test]
    fn ssh_tools_scene_shows_generate_and_saved_key() {
        let mut rt = runtime(state::scene_ssh_tools);
        rt.frame(1.0 / 60.0);
        for key in [
            "ssh-tools-form",
            "ssh-tool-tabs",
            "ssh-tool-name",
            "ssh-tool-algorithm",
            "ssh-tool-comment",
            "ssh-tool-generate",
            "ssh-tool-result-private",
            "ssh-tool-result-public",
            "ssh-tool-save",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn attachment_preview_scene_shows_text_and_actions() {
        let mut rt = runtime(state::scene_attachment_preview);
        rt.frame(1.0 / 60.0);
        for key in [
            "attachment-preview-screen",
            "attachment-preview-text",
            "attachment-preview-copy",
            "attachment-preview-close",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn sends_scene_shows_rows_and_actions() {
        let mut rt = runtime(state::scene_sends);
        rt.frame(1.0 / 60.0);
        for key in [
            "sends-screen",
            "send-list",
            "send-row-send-1",
            "send-detail",
            "send-copy-link-send-1",
            "send-disable-send-1",
            "send-delete-send-1",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        assert!(rt.find("send-disable-send-2").is_none());
    }

    #[test]
    fn send_form_scene_shows_all_inputs() {
        let mut rt = runtime(state::scene_send_form);
        rt.frame(1.0 / 60.0);
        for key in [
            "send-form",
            "send-name",
            "send-text",
            "send-hidden-toggle",
            "send-max-access",
            "send-expiration",
            "send-password",
            "send-submit",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn generator_submits_selected_options() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_generator(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("generator-length-increase").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("generator-generate").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::GeneratePassword { length: 21, .. }
        )));
    }

    #[test]
    fn generator_inserts_value_into_create_form() {
        let mut rt = runtime(state::scene_populated);
        rt.state.create = Some(state::start_create());
        state::open_generator(&mut rt.state, state::GeneratorTarget::Create);
        rt.frame(1.0 / 60.0);
        rt.state
            .generator
            .as_mut()
            .unwrap()
            .value
            .replace_text("generated-secret");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("generator-use").expect("use generated value");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.generator.is_none());
        assert_eq!(
            rt.state.create.as_ref().unwrap().password.text,
            "generated-secret"
        );
    }

    #[test]
    fn session_expired_scene_shows_account_form_and_notice() {
        let mut rt = runtime(state::scene_session_expired);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("account-form").is_some());
        assert!(rt.find("account-form-error").is_some());
        assert_eq!(
            rt.state.notice.as_deref(),
            Some("Your session expired. Sign in again.")
        );
    }

    #[test]
    fn account_scene_shows_connection_form() {
        let mut rt = runtime(state::scene_account);
        rt.frame(1.0 / 60.0);
        for key in [
            "account-form",
            "account-server",
            "account-modes",
            "account-mode-password",
            "account-mode-api",
            "account-email",
            "account-password",
            "account-submit",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let (x, y) = rt.find("account-submit").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("account-form-error").is_some());
        assert_eq!(
            rt.state.form_error.as_deref(),
            Some("Email and password are required.")
        );
    }

    #[test]
    fn account_form_submits_password_login() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_account(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        {
            let auth = rt.state.auth.as_mut().unwrap();
            auth.email.replace_text("user@example.com");
            auth.password.replace_text("master-secret");
        }
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("account-submit").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::LoginPassword {
                server_url,
                email,
                password,
                code,
                ..
            } if server_url == "https://vault.example.com"
                && email == "user@example.com"
                && password.as_str() == "master-secret"
                && code.is_none()
        )));
    }

    #[test]
    fn account_form_submits_api_key_login() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_account(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("account-mode-api").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("account-client-id").is_some());
        assert!(rt.find("account-client-secret").is_some());
        {
            let auth = rt.state.auth.as_mut().unwrap();
            auth.client_id.replace_text("user.123");
            auth.client_secret.replace_text("api-secret");
        }
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("account-submit").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::LoginApiKey {
                client_id,
                client_secret,
                ..
            } if client_id.as_str() == "user.123"
                && client_secret.as_str() == "api-secret"
        )));
    }

    #[test]
    fn fido2_form_submits_security_key_login() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_account_fido2(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.find("account-fido2-status").is_some());
        assert!(rt.find("account-security-key").is_some());
        let (x, y) = rt.find("account-fido2-continue").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::LoginPassword {
                two_factor_provider: Some(7),
                code: None,
                new_device_otp: None,
                ..
            }
        )));
    }

    #[test]
    fn two_factor_form_submits_verification_code() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_account_two_factor(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.find("account-code").is_some());
        assert!(rt.find("account-send-email").is_some());
        rt.state.auth.as_mut().unwrap().code.replace_text("123456");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("account-submit").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::LoginPassword {
                code: Some(code),
                two_factor_provider: Some(0),
                ..
            } if code.as_str() == "123456"
        )));
    }

    #[test]
    fn logout_is_dispatched_from_the_top_bar() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("logout").expect("logout button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, crate::backend::Request::Logout)));
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
            "detail-field-uri-1",
            "detail-field-password",
            "detail-field-totp",
            "detail-field-created",
            "detail-field-updated",
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
    fn detail_fields_copy_on_click_without_copy_buttons() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        for key in ["copy-user", "copy-password", "copy-uri-0"] {
            assert!(rt.find(key).is_none(), "unexpected button: {key}");
        }
        let (x, y) = rt.find("field-value-Username").expect("username field");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            backend::Request::CopyField { key, .. } if key == "username"
        )));
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
    fn shortcuts_edit_copy_and_open_settings() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx.clone());
        });
        rt.on_key = Some(on_key);
        rt.frame(1.0 / 60.0);
        let press = |rt: &mut Runtime<State>, key: Key| {
            rt.key(KeyEvent {
                key,
                mods: Mods::default(),
            });
            rt.frame(1.0 / 60.0);
        };

        press(&mut rt, Key::Text("e".to_owned()));
        assert!(rt.state.edit.is_some());
        assert!(rt.find("edit-name").is_some());
        press(&mut rt, Key::Escape);
        assert!(rt.state.edit.is_none());

        press(&mut rt, Key::Text("c".to_owned()));
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::CopyField {
                ref key,
                sensitive: true,
                ..
            } if key == "password"
        )));

        press(&mut rt, Key::Text(",".to_owned()));
        assert!(rt.state.settings_open);
        assert!(rt.find("settings").is_some());
        press(&mut rt, Key::Text("n".to_owned()));
        assert!(rt.state.create.is_none());
    }

    #[test]
    fn command_shortcuts_lock_and_sync() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx.clone());
        });
        rt.on_key = Some(on_key);
        rt.frame(1.0 / 60.0);
        for key in [Key::Command('l'), Key::Command('r')] {
            rt.key(KeyEvent {
                key,
                mods: Mods::default(),
            });
            rt.frame(1.0 / 60.0);
        }
        let requests: Vec<_> = rx.try_iter().collect();
        assert!(requests
            .iter()
            .any(|request| matches!(request, Request::Lock)));
        assert!(requests
            .iter()
            .any(|request| matches!(request, Request::Sync)));
    }

    #[test]
    fn single_letter_shortcuts_do_not_fire_while_typing() {
        let mut rt = runtime(state::scene_populated);
        rt.on_key = Some(on_key);
        rt.frame(1.0 / 60.0);
        rt.key(KeyEvent {
            key: Key::Text("/".to_owned()),
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
        rt.key(KeyEvent {
            key: Key::Text("e".to_owned()),
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.query.text.as_str(), "e");
        assert!(rt.state.edit.is_none());
    }

    #[test]
    fn settings_screen_shows_options_and_theme_choices() {
        let mut rt = runtime(state::scene_settings);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("settings").is_some());
        assert!(rt.find("settings-appearance").is_some());
        assert!(rt.find("settings-theme-system").is_some());
        assert!(rt.find("settings-theme-dark").is_some());
        assert!(rt.find("settings-theme-light").is_some());
        assert!(rt.find("settings-vault").is_some());
        assert!(rt.find("settings-auto-lock-value").is_some());
        assert!(rt.find("settings-clipboard-value").is_some());
        assert!(rt.find("settings-account").is_some());
        assert!(rt.find("field-value-Server").is_some());
        assert!(rt.find("settings-close").is_some());
    }

    #[test]
    fn settings_steppers_update_auto_lock_and_clipboard() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_settings(state);
            state.requests = Some(tx.clone());
        });
        rt.frame(1.0 / 60.0);

        let (x, y) = rt.find("settings-auto-lock-up").expect("auto-lock up");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        let (x, y) = rt.find("settings-clipboard-down").expect("clipboard down");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);

        let settings = rt.state.settings.as_ref().expect("settings");
        assert_eq!(settings.auto_lock_minutes, 30);
        assert_eq!(settings.clipboard_clear_seconds, 20);
        let requests: Vec<_> = rx.try_iter().collect();
        assert!(requests
            .iter()
            .any(|request| matches!(request, Request::SetAutoLock(30))));
        assert!(requests
            .iter()
            .any(|request| matches!(request, Request::SetClipboardClear(20))));
    }

    #[test]
    fn settings_theme_persists_and_escape_closes_settings() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx.clone());
        });
        rt.on_key = Some(on_key);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("settings").expect("settings button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.settings_open);

        let (x, y) = rt.find("settings-theme-light").expect("light theme button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.theme, ThemePreference::Light);
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::SetTheme(ThemePreference::Light))));

        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(!rt.state.settings_open);
        assert!(rt.find("details").is_some());
    }

    #[test]
    fn help_overlay_opens_and_closes() {
        let mut rt = runtime(state::scene_populated);
        rt.on_key = Some(on_key);
        rt.frame(1.0 / 60.0);
        rt.key(KeyEvent {
            key: Key::Text("?".to_owned()),
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.state.help);
        assert!(rt.find("help-overlay").is_some());
        assert!(rt.find("help-shortcut-0").is_some());
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(!rt.state.help);
        assert!(rt.find("details").is_some());
    }

    #[test]
    fn create_form_opens_and_submits_a_secure_note() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx.clone());
        });
        rt.on_key = Some(on_key);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in [
            "create-form",
            "create-name",
            "create-type-login",
            "create-type-note",
            "create-submit",
            "create-cancel",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }

        rt.state
            .create
            .as_mut()
            .unwrap()
            .name
            .replace_text("Recovery sheet");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-type-note").expect("note type button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("create-notes").is_some(), "missing notes field");
        rt.state
            .create
            .as_mut()
            .unwrap()
            .notes
            .replace_text("Keep this private");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-favorite").expect("favorite button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-folder-next").expect("folder button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let shown = rt
            .elements()
            .into_iter()
            .find(|element| element.key.as_deref() == Some("create-folder-label"))
            .expect("folder label")
            .text
            .expect("folder text");
        assert_eq!(shown, "Folder: Development");
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.create.is_none());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::CreateItem { name, item_type, favorite, folder_id, notes, .. }
                if name == "Recovery sheet" && item_type == 2 && favorite
                    && folder_id.as_deref() == Some("development")
                    && notes.as_ref().map(backend::Secret::as_str) == Some("Keep this private")
        )));
    }

    #[test]
    fn create_form_shows_validation_error() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("create-form-error").is_some());
        assert_eq!(
            rt.state.form_error.as_deref(),
            Some("Item name is required.")
        );
    }

    #[test]
    fn create_form_submits_login_credentials() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in [
            "create-login-fields",
            "create-username",
            "create-password",
            "create-uri-fields",
            "create-uri-0",
            "create-uri-add",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let draft = rt.state.create.as_mut().unwrap();
        draft.name.replace_text("Store");
        draft.username.replace_text("member@example.com");
        draft.password.replace_text("s3cret");
        draft.uris[0].replace_text("https://store.example.com");
        state::add_create_uri(&mut rt.state);
        rt.state.create.as_mut().unwrap().uris[1].replace_text("https://admin.store.example.com");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::CreateItem { username, password, uris, .. }
                if username.as_deref() == Some("member@example.com")
                    && password.as_ref().map(backend::Secret::as_str) == Some("s3cret")
                    && uris == vec![
                        "https://store.example.com",
                        "https://admin.store.example.com",
                    ]
        )));
    }

    #[test]
    fn create_form_submits_card_details() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-type-card").expect("card type button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in [
            "create-card-fields",
            "create-cardholder",
            "create-card-number",
            "create-card-brand",
            "create-card-exp-month",
            "create-card-exp-year",
            "create-card-code",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let draft = rt.state.create.as_mut().unwrap();
        draft.name.replace_text("Payment card");
        draft.card.cardholder_name.replace_text("Ada Lovelace");
        draft.card.number.replace_text("4242424242424242");
        draft.card.brand.replace_text("Visa");
        draft.card.exp_month.replace_text("12");
        draft.card.exp_year.replace_text("2030");
        draft.card.code.replace_text("123");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::CreateItem { card: Some(card), .. }
                if card.number.as_ref().map(backend::Secret::as_str) == Some("4242424242424242")
                    && card.code.as_ref().map(backend::Secret::as_str) == Some("123")
                    && card.exp_year.as_ref().map(backend::Secret::as_str) == Some("2030")
        )));
    }

    #[test]
    fn create_form_submits_identity_details() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt
            .find("create-type-identity")
            .expect("identity type button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in [
            "create-identity-fields",
            "create-first-name",
            "create-last-name",
            "create-email",
            "create-phone",
            "create-address1",
            "create-postal-code",
            "create-city",
            "create-country",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let draft = rt.state.create.as_mut().unwrap();
        draft.name.replace_text("Personal ID");
        draft.identity.first_name.replace_text("Ada");
        draft.identity.last_name.replace_text("Lovelace");
        draft.identity.email.replace_text("ada@example.com");
        draft.identity.phone.replace_text("+44 20 7946 0958");
        draft.identity.address1.replace_text("12 Analytical Way");
        draft.identity.postal_code.replace_text("SW1A 1AA");
        draft.identity.city.replace_text("London");
        draft.identity.country.replace_text("UK");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::CreateItem { item_type, identity: Some(identity), .. }
                if item_type == 4
                    && identity.first_name.as_ref().map(backend::Secret::as_str) == Some("Ada")
                    && identity.last_name.as_ref().map(backend::Secret::as_str) == Some("Lovelace")
                    && identity.email.as_ref().map(backend::Secret::as_str) == Some("ada@example.com")
                    && identity.city.as_ref().map(backend::Secret::as_str) == Some("London")
        )));
    }

    #[test]
    fn create_form_submits_ssh_key() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-type-ssh-key").expect("ssh key type button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in [
            "create-ssh-fields",
            "create-private-key",
            "create-public-key",
            "create-fingerprint",
            "create-notes",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let draft = rt.state.create.as_mut().unwrap();
        draft.name.replace_text("Deploy key");
        draft.ssh.private_key.replace_text(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----",
        );
        draft
            .ssh
            .public_key
            .replace_text("ssh-ed25519 AAAA deploy@host");
        draft.ssh.fingerprint.replace_text("SHA256:abc");
        draft.notes.replace_text("Production deploy key");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::CreateSshKey { name, private_key, public_key, fingerprint, notes, .. }
                if name == "Deploy key"
                    && private_key.as_str().contains("BEGIN OPENSSH PRIVATE KEY")
                    && public_key.as_deref() == Some("ssh-ed25519 AAAA deploy@host")
                    && fingerprint.as_deref() == Some("SHA256:abc")
                    && notes.as_ref().map(backend::Secret::as_str) == Some("Production deploy key")
        )));
    }

    #[test]
    fn create_form_submits_custom_fields() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("new-item").expect("new item button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("create-custom-fields").is_some());
        state::add_create_field(&mut rt.state);
        rt.frame(1.0 / 60.0);
        let draft = rt.state.create.as_mut().unwrap();
        draft.name.replace_text("With fields");
        draft.fields[0].name.replace_text("Recovery phrase");
        draft.fields[0]
            .value
            .replace_text("correct horse battery staple");
        draft.fields[0].field_type = 1;
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("create-submit").expect("create button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::CreateItem { name, fields, .. }
                if name == "With fields"
                    && fields.len() == 1
                    && fields[0].name == "Recovery phrase"
                    && fields[0].value.as_str() == "correct horse battery staple"
                    && fields[0].field_type == 1
        )));
    }

    #[test]
    fn edit_form_updates_login_credentials() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut rt = runtime(|state| {
            state::scene_populated(state);
            state.requests = Some(tx);
        });
        rt.frame(0.0);
        let (x, y) = rt.find("edit-item").expect("edit button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        for key in [
            "edit-login-fields",
            "edit-username",
            "edit-password",
            "edit-uri-fields",
            "edit-uri-0",
            "edit-uri-1",
            "edit-uri-add",
        ] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
        let draft = rt.state.edit.as_mut().unwrap();
        draft.password.replace_text("new-secret");
        draft.uris[0].replace_text("https://new.example.com");
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("save-edit").expect("save button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::UpdateItem { id, username, password, uris, .. }
                if id == "1"
                    && username.as_deref() == Some("dev@example.com")
                    && password.as_ref().map(backend::Secret::as_str) == Some("new-secret")
                    && uris == vec![
                        "https://new.example.com",
                        "https://docs.example.com",
                    ]
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
        let (x, y) = rt.find("edit-favorite").expect("favorite button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.find("save-edit").expect("save button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.edit.is_none());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            crate::backend::Request::UpdateItem { id, name, favorite, .. }
                if id == "1" && name == "Work GitHub" && !favorite
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
        assert!(rt.find("locked-bar").is_some());
        assert!(rt.find("top-bar").is_none());
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
