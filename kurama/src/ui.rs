use rune::edit::TextEdit;
use rune::tok::{self, type_};
use rune::{
    col, el, icon, row, text, text_input, Color, Cursor, Cx, El, GrowthSpec, Key, KeyEvent,
    ScrollSpec, Tx,
};

use crate::backend::{revealed_value, totp_value, FieldValue, Request, Secret, VaultRow};
use crate::icons;
use crate::state::{
    add_create_field, add_create_uri, add_edit_uri, adjust_generator_length,
    adjust_generator_words, cancel_create, cancel_delete_send, cancel_edit, cancel_send,
    close_attachment, close_generator, close_sends, confirm_delete_send, copy_field,
    copy_generated, copy_send, cycle_create_field_type, cycle_create_folder, cycle_edit_folder,
    delete_item, disable_send, dispatch, folder_label, generate_password, logout, move_selection,
    new_send, open_attachment, open_generator, open_sends, open_trash, remove_create_field,
    remove_create_uri, remove_edit_uri, request_delete_send, restore_item, select_category,
    select_folder, select_item, select_send, send_two_factor_email, set_auth_mode, set_create_type,
    set_favorite, set_generator_passphrase, start_edit, submit_auth, submit_create, submit_edit,
    submit_fido2, submit_quick_unlock, submit_search, submit_send, submit_unlock,
    toggle_create_favorite, toggle_create_field_value, toggle_edit_favorite,
    toggle_generator_option, toggle_reveal, toggle_send_hidden, use_generated, AttachmentView,
    AuthMode, CardDraft, Category, CustomFieldDraft, GeneratorOption, GeneratorTarget,
    IdentityDraft, Location, SendDraft, SshDraft, State,
};
use crate::theme::{
    accent, selected_row, META, ROW_SUBTITLE, ROW_TITLE, SECTION_TITLE, WINDOW_TITLE,
};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.dark(cx.system_dark());
    let t = tok::theme(dark);
    let connected = !s.snapshot.status.is_empty();
    let unlocked = s.snapshot.status == "unlocked";

    let content = if let Some(view) = s.attachment_preview.as_ref() {
        attachment_preview(s, view, dark)
    } else if s.generator.is_some() {
        generator(s, dark)
    } else if s.help {
        help(s, dark)
    } else if s.sends_open {
        sends(s, dark)
    } else if s.create.is_some() {
        create_form(s, dark)
    } else if s.edit.is_some() {
        edit_form(s, dark)
    } else if !connected {
        placeholder(
            s,
            dark,
            "Vault not connected",
            "Start the Kurama backend daemon to browse your vault.",
            "Retry",
            Request::Refresh,
        )
    } else if s.snapshot.status == "unauthenticated" {
        account(s, dark)
    } else if !unlocked {
        unlock(s, dark)
    } else {
        vault(s, dark)
    };

    col()
        .key("root")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(top_bar(s, dark))
        .child(content)
        .child(status_bar(s, dark))
}

fn help(_s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let shortcuts = [
        ("↑ / ↓ / j / k", "Move between items"),
        ("Home / End", "Select first or last item"),
        ("h / l", "Change category"),
        ("/", "Focus search"),
        ("n", "Create a new item"),
        ("g", "Generate a password"),
        ("s", "Open Sends"),
        ("?", "Toggle this help"),
        ("Esc", "Close a form or this help"),
    ];
    let mut list = col().key("help-list").gap(8.0);
    for (index, (keys, label)) in shortcuts.iter().enumerate() {
        list = list.child(
            row()
                .key(format!("help-shortcut-{index}"))
                .gap(16.0)
                .child(text(keys.to_owned()).font(type_::LABEL).color(t.text))
                .child(
                    text(label.to_owned())
                        .font(ROW_SUBTITLE)
                        .color(t.text_secondary),
                ),
        );
    }
    col()
        .key("help-overlay")
        .w_full()
        .grow(1.0)
        .h_full()
        .center()
        .gap(16.0)
        .fill(t.bg)
        .child(text("Keyboard shortcuts").font(WINDOW_TITLE).color(t.text))
        .child(list)
        .child(action("help-close", "Close", dark, |state| {
            state.help = false;
        }))
}

fn labeled_input(
    value: &TextEdit,
    key: &str,
    label: &str,
    placeholder: &str,
    dark: bool,
    on_edit: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
    submit: fn(&mut State),
) -> El {
    let t = tok::theme(dark);
    col()
        .gap(6.0)
        .child(
            text(label)
                .key(format!("{key}-label"))
                .font(SECTION_TITLE)
                .color(t.text_secondary)
                .h(18.0)
                .center_y(),
        )
        .child(
            text_input(value, type_::BODY)
                .key(key)
                .label(label)
                .placeholder(placeholder, t.text_secondary)
                .w_full()
                .h(40.0)
                .px(12.0)
                .radius(8.0)
                .fill(t.card)
                .border(1.0, t.card_border)
                .on_edit(on_edit)
                .on_submit(move |s: &mut State, _tx: &mut Tx| submit(s))
                .on_input_key(|s: &mut State, event: &KeyEvent, _tx: &mut Tx| {
                    if matches!(event.key, Key::Escape) {
                        if s.create.is_some() {
                            cancel_create(s);
                        } else if s.edit.is_some() {
                            cancel_edit(s);
                        } else if s.send_form.is_some() {
                            cancel_send(s);
                        } else {
                            return false;
                        }
                        true
                    } else {
                        false
                    }
                }),
        )
}

struct MultilineStyle<'a> {
    label: &'a str,
    placeholder: &'a str,
    height: f32,
    revealed_secure: bool,
}

fn multiline_field(
    value: &TextEdit,
    key: &str,
    style: MultilineStyle<'_>,
    dark: bool,
    on_edit: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
) -> El {
    let t = tok::theme(dark);
    let mut input = text_input(value, type_::BODY);
    if style.revealed_secure {
        input = input.secure().revealed(true);
    }
    col()
        .gap(6.0)
        .child(
            text(style.label)
                .font(SECTION_TITLE)
                .color(t.text_secondary)
                .h(18.0)
                .center_y(),
        )
        .child(
            input
                .key(key)
                .label(style.label)
                .placeholder(style.placeholder, t.text_secondary)
                .w_full()
                .h(style.height)
                .px(12.0)
                .radius(8.0)
                .fill(t.card)
                .border(1.0, t.card_border)
                .on_edit(on_edit)
                .on_input_key(|s: &mut State, event: &KeyEvent, _tx: &mut Tx| {
                    if matches!(event.key, Key::Escape) {
                        if s.create.is_some() {
                            cancel_create(s);
                        } else if s.edit.is_some() {
                            cancel_edit(s);
                        } else if s.send_form.is_some() {
                            cancel_send(s);
                        } else {
                            return false;
                        }
                        true
                    } else {
                        false
                    }
                }),
        )
}

fn notes_input(
    value: &TextEdit,
    key: &str,
    dark: bool,
    on_edit: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
) -> El {
    multiline_field(
        value,
        key,
        MultilineStyle {
            label: "Secure note",
            placeholder: "Write your note",
            height: 120.0,
            revealed_secure: false,
        },
        dark,
        on_edit,
    )
}

fn card_fields(
    card: &CardDraft,
    prefix: &str,
    dark: bool,
    draft: for<'a> fn(&'a mut State) -> &'a mut CardDraft,
    submit: fn(&mut State),
) -> El {
    col()
        .key(format!("{prefix}-card-fields"))
        .gap(16.0)
        .child(labeled_input(
            &card.cardholder_name,
            &format!("{prefix}-cardholder"),
            "Cardholder",
            "Name on card",
            dark,
            move |state| &mut draft(state).cardholder_name,
            submit,
        ))
        .child(labeled_input(
            &card.number,
            &format!("{prefix}-card-number"),
            "Number",
            "Card number",
            dark,
            move |state| &mut draft(state).number,
            submit,
        ))
        .child(
            row()
                .gap(14.0)
                .child(
                    labeled_input(
                        &card.brand,
                        &format!("{prefix}-card-brand"),
                        "Brand",
                        "Visa",
                        dark,
                        move |state| &mut draft(state).brand,
                        submit,
                    )
                    .grow(1.0),
                )
                .child(
                    labeled_input(
                        &card.exp_month,
                        &format!("{prefix}-card-exp-month"),
                        "Month",
                        "MM",
                        dark,
                        move |state| &mut draft(state).exp_month,
                        submit,
                    )
                    .w(84.0),
                )
                .child(
                    labeled_input(
                        &card.exp_year,
                        &format!("{prefix}-card-exp-year"),
                        "Year",
                        "YYYY",
                        dark,
                        move |state| &mut draft(state).exp_year,
                        submit,
                    )
                    .w(110.0),
                )
                .child(
                    labeled_input(
                        &card.code,
                        &format!("{prefix}-card-code"),
                        "Code",
                        "CVV",
                        dark,
                        move |state| &mut draft(state).code,
                        submit,
                    )
                    .w(100.0),
                ),
        )
}

fn form_field(
    key: impl AsRef<str>,
    label: &str,
    placeholder: &str,
    value: &TextEdit,
    dark: bool,
    on_edit: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
    submit: fn(&mut State),
) -> El {
    labeled_input(
        value,
        key.as_ref(),
        label,
        placeholder,
        dark,
        on_edit,
        submit,
    )
    .grow(1.0)
}

fn identity_fields(
    identity: &IdentityDraft,
    prefix: &str,
    dark: bool,
    draft: for<'a> fn(&'a mut State) -> &'a mut IdentityDraft,
    submit: fn(&mut State),
) -> El {
    col()
        .key(format!("{prefix}-identity-fields"))
        .gap(16.0)
        .child(
            row()
                .gap(12.0)
                .child(form_field(
                    format!("{prefix}-first-name"),
                    "First name",
                    "First name",
                    &identity.first_name,
                    dark,
                    move |state| &mut draft(state).first_name,
                    submit,
                ))
                .child(form_field(
                    format!("{prefix}-last-name"),
                    "Last name",
                    "Last name",
                    &identity.last_name,
                    dark,
                    move |state| &mut draft(state).last_name,
                    submit,
                )),
        )
        .child(
            row()
                .gap(12.0)
                .child(form_field(
                    format!("{prefix}-email"),
                    "Email",
                    "name@example.com",
                    &identity.email,
                    dark,
                    move |state| &mut draft(state).email,
                    submit,
                ))
                .child(form_field(
                    format!("{prefix}-phone"),
                    "Phone",
                    "Phone number",
                    &identity.phone,
                    dark,
                    move |state| &mut draft(state).phone,
                    submit,
                )),
        )
        .child(
            row()
                .gap(12.0)
                .child(form_field(
                    format!("{prefix}-address1"),
                    "Address",
                    "Street address",
                    &identity.address1,
                    dark,
                    move |state| &mut draft(state).address1,
                    submit,
                ))
                .child(form_field(
                    format!("{prefix}-postal-code"),
                    "Postal code",
                    "Postal code",
                    &identity.postal_code,
                    dark,
                    move |state| &mut draft(state).postal_code,
                    submit,
                )),
        )
        .child(
            row()
                .gap(12.0)
                .child(form_field(
                    format!("{prefix}-city"),
                    "City",
                    "City",
                    &identity.city,
                    dark,
                    move |state| &mut draft(state).city,
                    submit,
                ))
                .child(form_field(
                    format!("{prefix}-country"),
                    "Country",
                    "Country",
                    &identity.country,
                    dark,
                    move |state| &mut draft(state).country,
                    submit,
                )),
        )
}

fn ssh_fields(ssh: &SshDraft, dark: bool, submit: fn(&mut State)) -> El {
    col()
        .key("create-ssh-fields")
        .gap(16.0)
        .child(multiline_field(
            &ssh.private_key,
            "create-private-key",
            MultilineStyle {
                label: "Private key",
                placeholder: "-----BEGIN OPENSSH PRIVATE KEY-----",
                height: 150.0,
                revealed_secure: true,
            },
            dark,
            |state| &mut state.create.as_mut().unwrap().ssh.private_key,
        ))
        .child(multiline_field(
            &ssh.public_key,
            "create-public-key",
            MultilineStyle {
                label: "Public key",
                placeholder: "ssh-ed25519 AAAA...",
                height: 90.0,
                revealed_secure: false,
            },
            dark,
            |state| &mut state.create.as_mut().unwrap().ssh.public_key,
        ))
        .child(form_field(
            "create-fingerprint",
            "Fingerprint",
            "SHA256:...",
            &ssh.fingerprint,
            dark,
            |state| &mut state.create.as_mut().unwrap().ssh.fingerprint,
            submit,
        ))
}

fn custom_fields(fields: &[CustomFieldDraft], dark: bool) -> El {
    let t = tok::theme(dark);
    let mut view = col().key("create-custom-fields").gap(12.0).child(
        text("Custom fields")
            .font(SECTION_TITLE)
            .color(t.text_secondary)
            .h(18.0)
            .center_y(),
    );
    for (index, field) in fields.iter().enumerate() {
        let type_label = match field.field_type {
            1 => "Hidden",
            2 => "Boolean",
            _ => "Text",
        };
        let value = if field.field_type == 2 {
            let enabled = field.value.text.eq_ignore_ascii_case("true") || field.value.text == "1";
            let label = if enabled { "Yes" } else { "No" };
            labeled_input(
                &field.value,
                &format!("create-field-value-{index}"),
                "Value",
                "false",
                dark,
                move |state| &mut state.create.as_mut().unwrap().fields[index].value,
                submit_create,
            )
            .child(action(
                &format!("create-field-toggle-{index}"),
                label,
                dark,
                move |state| toggle_create_field_value(state, index),
            ))
        } else {
            labeled_input(
                &field.value,
                &format!("create-field-value-{index}"),
                "Value",
                "Value",
                dark,
                move |state| &mut state.create.as_mut().unwrap().fields[index].value,
                submit_create,
            )
        };
        view = view.child(
            row()
                .key(format!("create-field-row-{index}"))
                .gap(12.0)
                .center_y()
                .child(
                    labeled_input(
                        &field.name,
                        &format!("create-field-name-{index}"),
                        "Name",
                        "Field name",
                        dark,
                        move |state| &mut state.create.as_mut().unwrap().fields[index].name,
                        submit_create,
                    )
                    .w(180.0),
                )
                .child(value.grow(1.0))
                .child(
                    col()
                        .gap(6.0)
                        .child(
                            text("Type")
                                .font(SECTION_TITLE)
                                .color(t.text_secondary)
                                .h(18.0)
                                .center_y(),
                        )
                        .child(
                            row()
                                .gap(6.0)
                                .child(action(
                                    &format!("create-field-type-{index}"),
                                    type_label,
                                    dark,
                                    move |state| cycle_create_field_type(state, index),
                                ))
                                .child(action(
                                    &format!("create-field-remove-{index}"),
                                    "×",
                                    dark,
                                    move |state| remove_create_field(state, index),
                                )),
                        ),
                ),
        );
    }
    view.child(action(
        "create-field-add",
        "+ Add field",
        dark,
        add_create_field,
    ))
}

fn create_uri_fields(uris: &[TextEdit], dark: bool) -> El {
    let t = tok::theme(dark);
    let mut view = col().key("create-uri-fields").gap(12.0).child(
        text("Websites")
            .font(SECTION_TITLE)
            .color(t.text_secondary)
            .h(18.0)
            .center_y(),
    );
    for (index, uri) in uris.iter().enumerate() {
        let label = if uris.len() == 1 {
            "Website".to_owned()
        } else {
            format!("Website {}", index + 1)
        };
        view = view.child(
            row()
                .key(format!("create-uri-row-{index}"))
                .gap(10.0)
                .center_y()
                .child(form_field(
                    format!("create-uri-{index}"),
                    &label,
                    "https://example.com",
                    uri,
                    dark,
                    move |state| &mut state.create.as_mut().unwrap().uris[index],
                    submit_create,
                ))
                .child(
                    action(
                        &format!("create-uri-remove-{index}"),
                        "×",
                        dark,
                        move |state| remove_create_uri(state, index),
                    )
                    .w(32.0),
                ),
        );
    }
    view.child(action(
        "create-uri-add",
        "+ Add website",
        dark,
        add_create_uri,
    ))
}

fn edit_uri_fields(uris: &[TextEdit], dark: bool) -> El {
    let t = tok::theme(dark);
    let mut view = col().key("edit-uri-fields").gap(12.0).child(
        text("Websites")
            .font(SECTION_TITLE)
            .color(t.text_secondary)
            .h(18.0)
            .center_y(),
    );
    for (index, uri) in uris.iter().enumerate() {
        let label = if uris.len() == 1 {
            "Website".to_owned()
        } else {
            format!("Website {}", index + 1)
        };
        view = view.child(
            row()
                .key(format!("edit-uri-row-{index}"))
                .gap(10.0)
                .center_y()
                .child(form_field(
                    format!("edit-uri-{index}"),
                    &label,
                    "https://example.com",
                    uri,
                    dark,
                    move |state| &mut state.edit.as_mut().unwrap().uris[index],
                    submit_edit,
                ))
                .child(
                    action(
                        &format!("edit-uri-remove-{index}"),
                        "×",
                        dark,
                        move |state| remove_edit_uri(state, index),
                    )
                    .w(32.0),
                ),
        );
    }
    view.child(action("edit-uri-add", "+ Add website", dark, add_edit_uri))
}

fn form_panel(
    key: &str,
    title: &str,
    dark: bool,
    header_extra: El,
    body: El,
    footer: El,
    error: Option<&str>,
) -> El {
    let t = tok::theme(dark);
    col()
        .key(key)
        .w_full()
        .grow(1.0)
        .h_full()
        .center()
        .padding(20.0)
        .fill(t.bg)
        .child(
            col()
                .key(format!("{key}-panel"))
                .w(680.0)
                .max_h(620.0)
                .radius(14.0)
                .fill(t.card)
                .border(1.0, t.card_border)
                .child(
                    col()
                        .key(format!("{key}-header"))
                        .gap(12.0)
                        .padding(20.0)
                        .child(text(title).font(WINDOW_TITLE).color(t.text))
                        .child(header_extra),
                )
                .child(
                    body.key(format!("{key}-body"))
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .shrink()
                        .min_h(0.0)
                        .padding(20.0)
                        .gap(16.0),
                )
                .child(if let Some(error) = error {
                    text(error.to_owned())
                        .key(format!("{key}-error"))
                        .font(ROW_SUBTITLE)
                        .color(Color {
                            r: 0.96,
                            g: 0.45,
                            b: 0.42,
                            a: 1.0,
                        })
                        .w_full()
                        .px(20.0)
                        .pb(8.0)
                } else {
                    el()
                })
                .child(el().h(1.0).w_full().fill(t.separator))
                .child(
                    row()
                        .key(format!("{key}-footer"))
                        .w_full()
                        .gap(16.0)
                        .center_y()
                        .padding(18.0)
                        .child(footer.grow(1.0)),
                ),
        )
}

fn create_form(s: &State, dark: bool) -> El {
    let Some(draft) = s.create.as_ref() else {
        return el();
    };
    let types = [
        ("login", "Login", 1),
        ("note", "Secure Note", 2),
        ("card", "Card", 3),
        ("identity", "Identity", 4),
        ("ssh-key", "SSH Key", 5),
    ];
    let mut choices = row().key("create-types").gap(12.0);
    for (slug, label, value) in types {
        let active = draft.item_type == value;
        choices = choices.child(action(
            &format!("create-type-{slug}"),
            if active {
                format!("• {label}")
            } else {
                label.to_owned()
            },
            dark,
            move |state| set_create_type(state, value),
        ));
    }

    let body = col()
        .child(labeled_input(
            &draft.name,
            "create-name",
            "Item name",
            "Item name",
            dark,
            |state| &mut state.create.as_mut().unwrap().name,
            submit_create,
        ))
        .child(if draft.item_type == 1 {
            col()
                .key("create-login-fields")
                .gap(16.0)
                .child(labeled_input(
                    &draft.username,
                    "create-username",
                    "Username",
                    "Username",
                    dark,
                    |state| &mut state.create.as_mut().unwrap().username,
                    submit_create,
                ))
                .child(labeled_input(
                    &draft.password,
                    "create-password",
                    "Password",
                    "Password",
                    dark,
                    |state| &mut state.create.as_mut().unwrap().password,
                    submit_create,
                ))
                .child(action(
                    "create-generate",
                    "Generate password",
                    dark,
                    |state| open_generator(state, GeneratorTarget::Create),
                ))
                .child(create_uri_fields(&draft.uris, dark))
        } else {
            el()
        })
        .child(if draft.item_type == 3 {
            card_fields(
                &draft.card,
                "create",
                dark,
                |state| &mut state.create.as_mut().unwrap().card,
                submit_create,
            )
        } else {
            el()
        })
        .child(if draft.item_type == 4 {
            identity_fields(
                &draft.identity,
                "create",
                dark,
                |state| &mut state.create.as_mut().unwrap().identity,
                submit_create,
            )
        } else {
            el()
        })
        .child(if draft.item_type == 5 {
            ssh_fields(&draft.ssh, dark, submit_create)
        } else {
            el()
        })
        .child(if draft.item_type == 2 || draft.item_type == 5 {
            notes_input(&draft.notes, "create-notes", dark, |state| {
                &mut state.create.as_mut().unwrap().notes
            })
        } else {
            el()
        })
        .child(custom_fields(&draft.fields, dark));

    let footer = row()
        .child(action(
            "create-favorite",
            if draft.favorite {
                "★ Favorite"
            } else {
                "☆ Favorite"
            },
            dark,
            toggle_create_favorite,
        ))
        .child(
            row()
                .key("create-folder")
                .gap(10.0)
                .center_y()
                .child(action("create-folder-prev", "‹", dark, |state| {
                    cycle_create_folder(state, -1)
                }))
                .child(
                    text(format!(
                        "Folder: {}",
                        folder_label(s, draft.folder_id.as_deref())
                    ))
                    .key("create-folder-label")
                    .font(ROW_SUBTITLE)
                    .color(tok::theme(dark).text),
                )
                .child(action("create-folder-next", "›", dark, |state| {
                    cycle_create_folder(state, 1)
                })),
        )
        .child(el().grow(1.0))
        .child(action("create-submit", "Create", dark, submit_create))
        .child(action("create-cancel", "Cancel", dark, cancel_create));

    form_panel(
        "create-form",
        "New item",
        dark,
        choices,
        body,
        footer,
        s.form_error.as_deref(),
    )
}

fn edit_form(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let Some(draft) = s.edit.as_ref() else {
        return el();
    };
    let type_label = match draft.item_type {
        1 => "Login",
        2 => "Secure Note",
        3 => "Card",
        4 => "Identity",
        5 => "SSH Key",
        _ => "Item",
    };
    let header = text(format!("Type: {type_label}"))
        .font(ROW_SUBTITLE)
        .color(t.text_secondary);

    let body = col()
        .child(labeled_input(
            &draft.name,
            "edit-name",
            "Item name",
            "Item name",
            dark,
            |state| &mut state.edit.as_mut().unwrap().name,
            submit_edit,
        ))
        .child(if draft.login {
            col()
                .key("edit-login-fields")
                .gap(16.0)
                .child(labeled_input(
                    &draft.username,
                    "edit-username",
                    "Username",
                    "Username",
                    dark,
                    |state| &mut state.edit.as_mut().unwrap().username,
                    submit_edit,
                ))
                .child(labeled_input(
                    &draft.password,
                    "edit-password",
                    "Password",
                    "Leave blank to keep",
                    dark,
                    |state| &mut state.edit.as_mut().unwrap().password,
                    submit_edit,
                ))
                .child(action(
                    "edit-generate",
                    "Generate password",
                    dark,
                    |state| open_generator(state, GeneratorTarget::Edit),
                ))
                .child(edit_uri_fields(&draft.uris, dark))
        } else {
            el()
        })
        .child(if draft.item_type == 3 {
            card_fields(
                &draft.card,
                "edit",
                dark,
                |state| &mut state.edit.as_mut().unwrap().card,
                submit_edit,
            )
        } else {
            el()
        })
        .child(if draft.item_type == 4 {
            identity_fields(
                &draft.identity,
                "edit",
                dark,
                |state| &mut state.edit.as_mut().unwrap().identity,
                submit_edit,
            )
        } else {
            el()
        })
        .child(if draft.item_type == 5 {
            text("Key material can only be set when importing.")
                .font(ROW_SUBTITLE)
                .color(t.text_secondary)
        } else {
            el()
        })
        .child(if draft.item_type == 2 || draft.item_type == 5 {
            notes_input(&draft.notes, "edit-notes", dark, |state| {
                &mut state.edit.as_mut().unwrap().notes
            })
        } else {
            el()
        });

    let footer = row()
        .child(action(
            "edit-favorite",
            if draft.favorite {
                "★ Favorite"
            } else {
                "☆ Favorite"
            },
            dark,
            toggle_edit_favorite,
        ))
        .child(
            row()
                .key("edit-folder")
                .gap(10.0)
                .center_y()
                .child(action("edit-folder-prev", "‹", dark, |state| {
                    cycle_edit_folder(state, -1)
                }))
                .child(
                    text(format!(
                        "Folder: {}",
                        folder_label(s, draft.folder_id.as_deref())
                    ))
                    .key("edit-folder-label")
                    .font(ROW_SUBTITLE)
                    .color(t.text),
                )
                .child(action("edit-folder-next", "›", dark, |state| {
                    cycle_edit_folder(state, 1)
                })),
        )
        .child(el().grow(1.0))
        .child(action("save-edit", "Save", dark, submit_edit))
        .child(action("cancel-edit", "Cancel", dark, cancel_edit));

    form_panel(
        "edit-form",
        "Edit item",
        dark,
        header,
        body,
        footer,
        s.form_error.as_deref(),
    )
}

fn attachment_preview(_s: &State, view: &AttachmentView, dark: bool) -> El {
    let t = tok::theme(dark);
    let preview_text = view
        .is_text
        .then(|| crate::backend::attachment_text(&view.filename))
        .flatten();
    let preview_edit = TextEdit::new(preview_text.clone().unwrap_or_default());
    let content = if preview_text.is_some() {
        text_input(&preview_edit, type_::BODY)
            .secure()
            .revealed(true)
            .key("attachment-preview-text")
            .label("Attachment text preview")
            .w_full()
            .grow(1.0)
            .min_h(160.0)
            .padding(12.0)
            .radius(8.0)
            .fill(t.card)
            .border(1.0, t.card_border)
            .color(t.text)
    } else {
        col()
            .key("attachment-preview-message")
            .grow(1.0)
            .center()
            .gap(8.0)
            .child(
                text(if view.is_image {
                    "Image preview opened in the default viewer."
                } else if view.preview {
                    "Preview opened in the default application."
                } else {
                    "Attachment saved to the local download folder."
                })
                .font(ROW_SUBTITLE)
                .color(t.text),
            )
            .child(
                text(&view.path)
                    .font(META)
                    .color(t.text_secondary)
                    .truncate(),
            )
    };
    let filename = view.filename.clone();
    let copy = preview_text.is_some().then(|| {
        action("attachment-preview-copy", "Copy text", dark, move |state| {
            if let Some(text) = crate::backend::attachment_text(&filename) {
                dispatch(
                    state,
                    Request::CopyText {
                        text: Secret::new(text),
                        label: "Attachment text".to_owned(),
                    },
                );
            }
        })
    });
    let mut footer = row().gap(8.0);
    if let Some(copy) = copy {
        footer = footer.child(copy);
    }
    footer = footer.child(action(
        "attachment-preview-close",
        "Close",
        dark,
        close_attachment,
    ));
    col()
        .key("attachment-preview-screen")
        .w_full()
        .grow(1.0)
        .h_full()
        .padding(20.0)
        .gap(14.0)
        .fill(t.bg)
        .child(
            row().w_full().gap(12.0).child(
                text(format!("Attachment · {}", view.filename))
                    .font(WINDOW_TITLE)
                    .color(t.text),
            ),
        )
        .child(field("Saved to", &view.path, dark))
        .child(content)
        .child(footer)
}

fn sends(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    if let Some(draft) = s.send_form.as_ref() {
        return send_form(s, draft, dark);
    }

    let header = row()
        .key("sends-header")
        .w_full()
        .gap(10.0)
        .child(text("Sends").font(WINDOW_TITLE).color(t.text))
        .child(el().grow(1.0))
        .child(action("sends-back", "Back", dark, close_sends))
        .child(action("sends-refresh", "Refresh", dark, |s| {
            dispatch(s, Request::ListSends)
        }))
        .child(action("sends-new", "New send", dark, new_send));

    let mut list = col()
        .key("send-list")
        .w(300.0)
        .h_full()
        .padding(12.0)
        .gap(4.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    for row in &s.sends {
        let active = s.send_selected.as_deref() == Some(row.id.as_str());
        let id = row.id.clone();
        let status = if row.disabled { "Disabled" } else { "Active" };
        list = list.child(
            el().key(format!("send-row-{}", row.id))
                .h(58.0)
                .px(10.0)
                .radius(8.0)
                .role("button")
                .label(format!("{}, {status}", row.name))
                .checked(active)
                .focusable(true)
                .fill(if active {
                    selected_row(dark)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .press(t.press)
                .cursor(Cursor::Pointer)
                .on_press(move |state: &mut State| select_send(state, id.clone()))
                .child(
                    col()
                        .gap(3.0)
                        .child(
                            text(row.name.clone())
                                .font(ROW_TITLE)
                                .color(t.text)
                                .truncate(),
                        )
                        .child(text(status).font(META).color(if row.disabled {
                            t.text_secondary
                        } else {
                            accent(dark)
                        })),
                ),
        );
    }

    let detail = match s
        .sends
        .iter()
        .find(|row| s.send_selected.as_deref() == Some(row.id.as_str()))
    {
        Some(send) => {
            let id = send.id.clone();
            let copy_id = id.clone();
            let disable_id = id.clone();
            let delete_id = id.clone();
            let has_link = crate::backend::send_url(&id).is_some();
            let mut view = col()
                .key("send-detail")
                .grow(1.0)
                .gap(14.0)
                .padding(20.0)
                .fill(t.bg)
                .child(text(send.name.clone()).font(WINDOW_TITLE).color(t.text))
                .child(field(
                    "Status",
                    if send.disabled { "Disabled" } else { "Active" },
                    dark,
                ))
                .child(field(
                    "Expires",
                    if send.expiration.is_empty() {
                        "Never"
                    } else {
                        &send.expiration
                    },
                    dark,
                ))
                .child(field(
                    "Accesses",
                    &if send.max_access == 0 {
                        format!("{} / unlimited", send.access_count)
                    } else {
                        format!("{} / {}", send.access_count, send.max_access)
                    },
                    dark,
                ))
                .child(field(
                    "Link",
                    if has_link {
                        "Available (copy to clipboard)"
                    } else {
                        "Unavailable"
                    },
                    dark,
                ))
                .child(row().gap(8.0).child(action(
                    &format!("send-copy-link-{id}"),
                    "Copy link",
                    dark,
                    move |state| copy_send(state, copy_id.clone()),
                )));
            if !send.disabled {
                view = view.child(action(
                    &format!("send-disable-{disable_id}"),
                    "Disable send",
                    dark,
                    move |state| disable_send(state, disable_id.clone()),
                ));
            }
            view = view.child(action(
                &format!("send-delete-{delete_id}"),
                "Delete send",
                dark,
                move |state| request_delete_send(state, delete_id.clone()),
            ));
            if s.send_pending_delete.as_deref() == Some(send.id.as_str()) {
                view = view.child(
                    col()
                        .key("send-delete-confirm")
                        .gap(8.0)
                        .padding(12.0)
                        .radius(8.0)
                        .fill(t.card)
                        .child(text("Delete this send permanently?").color(t.text))
                        .child(
                            row()
                                .gap(8.0)
                                .child(action(
                                    "send-delete-cancel",
                                    "Cancel",
                                    dark,
                                    cancel_delete_send,
                                ))
                                .child(action(
                                    "send-delete-confirm-button",
                                    "Delete",
                                    dark,
                                    confirm_delete_send,
                                )),
                        ),
                );
            }
            view
        }
        None => col()
            .key("send-detail")
            .grow(1.0)
            .center()
            .gap(8.0)
            .fill(t.bg)
            .child(text("No send selected").font(WINDOW_TITLE).color(t.text))
            .child(
                text(if s.sends.is_empty() {
                    s.send_error.as_deref().unwrap_or("No sends yet.")
                } else {
                    "Select a send to see its details."
                })
                .font(ROW_SUBTITLE)
                .color(t.text_secondary),
            ),
    };

    col()
        .key("sends-screen")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .padding(16.0)
        .gap(12.0)
        .fill(t.bg)
        .child(header)
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(12.0)
                .child(list)
                .child(detail),
        )
}

fn send_form(s: &State, draft: &SendDraft, dark: bool) -> El {
    let header_extra = row()
        .gap(8.0)
        .child(action("send-form-back", "Back", dark, cancel_send));
    let body = col()
        .gap(14.0)
        .child(labeled_input(
            &draft.name,
            "send-name",
            "Name",
            "Optional send name",
            dark,
            |state| &mut state.send_form.as_mut().unwrap().name,
            submit_send,
        ))
        .child(multiline_field(
            &draft.text,
            "send-text",
            MultilineStyle {
                label: "Text to share",
                placeholder: "Paste the text to share",
                height: 120.0,
                revealed_secure: true,
            },
            dark,
            |state| &mut state.send_form.as_mut().unwrap().text,
        ))
        .child(action(
            "send-hidden-toggle",
            if draft.hidden {
                "Hide text after reading: On"
            } else {
                "Hide text after reading: Off"
            },
            dark,
            toggle_send_hidden,
        ))
        .child(
            row()
                .gap(12.0)
                .child(labeled_input(
                    &draft.max_access,
                    "send-max-access",
                    "Maximum accesses",
                    "Unlimited",
                    dark,
                    |state| &mut state.send_form.as_mut().unwrap().max_access,
                    submit_send,
                ))
                .child(labeled_input(
                    &draft.expiration,
                    "send-expiration",
                    "Expires at",
                    "ISO 8601 date",
                    dark,
                    |state| &mut state.send_form.as_mut().unwrap().expiration,
                    submit_send,
                )),
        )
        .child(labeled_input(
            &draft.password,
            "send-password",
            "Password (optional)",
            "Require a password to open",
            dark,
            |state| &mut state.send_form.as_mut().unwrap().password,
            submit_send,
        ));
    let footer = row()
        .gap(8.0)
        .child(action("send-cancel", "Cancel", dark, cancel_send))
        .child(action("send-submit", "Create send", dark, submit_send));
    form_panel(
        "send-form",
        "New send",
        dark,
        header_extra,
        body,
        footer,
        s.form_error.as_deref(),
    )
}

fn top_bar(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let search = text_input(&s.query, type_::BODY)
        .key("search")
        .label("Search items")
        .placeholder("Search items", t.text_secondary)
        .color(t.text)
        .h(34.0)
        .w(320.0)
        .padding(10.0)
        .radius(8.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .on_edit(|s: &mut State| &mut s.query)
        .on_submit(submit_search)
        .on_input_key(
            |s: &mut State, event: &KeyEvent, tx: &mut Tx| match &event.key {
                Key::Down => move_selection(s, 1, tx),
                Key::Up => move_selection(s, -1, tx),
                Key::Enter => {
                    submit_search(s, tx);
                    true
                }
                _ => false,
            },
        );

    let brand = text("Kurama").key("brand").font(WINDOW_TITLE).color(t.text);

    row()
        .key("top-bar")
        .w_full()
        .h(64.0)
        .px(18.0)
        .gap(16.0)
        .center_y()
        .fill(t.card)
        .border(1.0, t.card_border)
        .child(brand)
        .child(search)
        .child(el().grow(1.0))
        .child(action_tx("new-item", "New", dark, |state, tx| {
            state.create = Some(crate::state::start_create());
            state.form_error = None;
            tx.focus("create-name", true);
        }))
        .child(action(
            "theme",
            if dark { "Light" } else { "Dark" },
            dark,
            move |s| {
                s.dark_override = Some(!dark);
            },
        ))
        .child(action("sends", "Sends", dark, open_sends))
        .child(action("sync", "Sync", dark, |s| dispatch(s, Request::Sync)))
        .child(action("generate", "Generate", dark, |s| {
            open_generator(s, GeneratorTarget::None)
        }))
        .child(action("?", "?", dark, |s| s.help = !s.help))
        .child(action("lock", "Lock", dark, |s| dispatch(s, Request::Lock)))
        .child(if s.snapshot.status == "unlocked" {
            action("logout", "Log out", dark, logout)
        } else {
            el()
        })
}

fn vault(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let body = row()
        .w_full()
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .child(sidebar(s, dark))
        .child(item_list(s, dark))
        .child(details(s, dark))
        .fill(t.bg);

    if let Some(approval) = s.approvals.first() {
        let title = if approval.title.is_empty() {
            &approval.action
        } else {
            &approval.title
        };
        let banner = row()
            .key("approvals-banner")
            .w_full()
            .h(36.0)
            .px(14.0)
            .center_y()
            .fill(t.control_track)
            .child(
                text(format!("SSH approval requested · {title}"))
                    .font(type_::LABEL)
                    .color(t.text),
            );
        return col()
            .key("vault")
            .w_full()
            .grow(1.0)
            .shrink()
            .min_h(0.0)
            .child(banner)
            .child(body);
    }
    body.key("vault")
}

fn sidebar(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let mut nav = col()
        .key("sidebar")
        .w(190.0)
        .h_full()
        .padding(12.0)
        .gap(3.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .child(
            text("LIBRARY")
                .key("library-label")
                .font(SECTION_TITLE)
                .color(t.text_secondary)
                .padding(8.0),
        );

    for category in Category::ALL {
        let active = s.category == category;
        let label = category.label();
        let count = match category {
            Category::All => s.rows.len(),
            Category::Favorites => s.rows.iter().filter(|item| item.favorite).count(),
            _ => s.rows.iter().filter(|item| category.matches(item)).count(),
        };
        nav = nav.child(
            el().key(format!("category-{label}"))
                .h(34.0)
                .px(10.0)
                .radius(8.0)
                .role("button")
                .label(label)
                .checked(active)
                .focusable(true)
                .fill(if active {
                    selected_row(dark)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .press(t.press)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| select_category(s, category))
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .gap(8.0)
                        .child(
                            text(label)
                                .font(type_::LABEL)
                                .color(if active { accent(dark) } else { t.text })
                                .grow(1.0),
                        )
                        .child(text(count.to_string()).font(META).color(t.text_secondary)),
                ),
        );
    }

    nav = nav.child(
        text("FOLDERS")
            .key("folders-label")
            .font(SECTION_TITLE)
            .color(t.text_secondary)
            .padding(8.0),
    );
    let mut folders = col()
        .key("folder-list")
        .gap(2.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .shrink()
        .min_h(0.0);
    if s.folders.is_empty() {
        folders = folders.child(
            text("No folders")
                .font(META)
                .color(t.text_secondary)
                .padding(10.0),
        );
    }
    for folder in &s.folders {
        let active = s.location != Location::Trash && s.folder.as_deref() == Some(folder.as_str());
        let target = folder.clone();
        folders = folders.child(
            el().key(format!("folder-{folder}"))
                .h(30.0)
                .px(10.0)
                .radius(8.0)
                .role("button")
                .label(folder.clone())
                .checked(active)
                .focusable(true)
                .fill(if active {
                    selected_row(dark)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .press(t.press)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| select_folder(s, target.clone()))
                .child(
                    text(folder.clone())
                        .font(type_::LABEL)
                        .color(if active { accent(dark) } else { t.text })
                        .truncate(),
                ),
        );
    }
    nav = nav.child(folders).child(
        el().key("trash-nav")
            .h(34.0)
            .px(10.0)
            .radius(8.0)
            .role("button")
            .label("Trash")
            .checked(s.location == Location::Trash)
            .focusable(true)
            .fill(if s.location == Location::Trash {
                selected_row(dark)
            } else {
                Color::TRANSPARENT
            })
            .hover(t.hover)
            .press(t.press)
            .cursor(Cursor::Pointer)
            .on_press(|s: &mut State| open_trash(s))
            .child(
                text("Trash")
                    .font(type_::LABEL)
                    .color(if s.location == Location::Trash {
                        accent(dark)
                    } else {
                        t.text
                    }),
            ),
    );

    nav.child(el().grow(1.0))
}

fn item_list(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let visible_count = s.visible_indices().len();
    let version = (
        s.rows_version(),
        s.query.text.clone(),
        s.category,
        s.location,
        s.folder.clone(),
    );
    let list = col()
        .key("item-rows")
        .w_full()
        .padding(8.0)
        .gap(2.0)
        .growth(GrowthSpec::default())
        .virtual_rows(
            visible_count,
            58.0,
            |s: &State, index: usize| {
                let row = s.visible_indices()[index];
                format!("item-{}", s.rows[row].id).into()
            },
            move |s: &State, index: usize| {
                let row = s.visible_indices()[index];
                let item = &s.rows[row];
                item_row(item, s.selected.as_deref() == Some(item.id.as_str()), dark)
            },
        )
        .virtual_rows_version(version);

    let empty = if visible_count == 0 {
        el().key("list-empty").padding(24.0).child(
            text("No items match this view.")
                .font(ROW_SUBTITLE)
                .color(t.text_secondary),
        )
    } else {
        el()
    };

    let title = if s.location == Location::Trash {
        "Trash"
    } else if let Some(folder) = &s.folder {
        folder.as_str()
    } else {
        s.category.label()
    };
    col()
        .key("items")
        .w(292.0)
        .h_full()
        .fill(t.card)
        .border(1.0, t.card_border)
        .child(
            row()
                .key("items-header")
                .w_full()
                .h(42.0)
                .px(14.0)
                .center_y()
                .child(
                    text(title.to_owned())
                        .font(SECTION_TITLE)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    text(visible_count.to_string())
                        .font(META)
                        .color(t.text_secondary),
                ),
        )
        .child(el().h(1.0).w_full().fill(t.separator))
        .child(if visible_count == 0 {
            empty
        } else {
            col()
                .key("item-scroller")
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .shrink()
                .min_h(0.0)
                .child(list)
        })
}

fn item_row(item: &VaultRow, selected: bool, dark: bool) -> El {
    let t = tok::theme(dark);
    let id = item.id.clone();
    let star = if item.favorite { "★" } else { "" };
    let type_label = icons::label(&item.type_name);

    el().key(format!("item-{}", item.id))
        .h(56.0)
        .w_full()
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(8.0)
        .role("listitem")
        .label(format!("{}, {}", item.name, item.subtitle))
        .checked(selected)
        .focusable(true)
        .fill(if selected {
            selected_row(dark)
        } else {
            Color::TRANSPARENT
        })
        .hover(t.hover)
        .press(t.press)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| select_item(s, id.clone()))
        .child(
            icon(icons::for_type(&item.type_name), 18.0)
                .key(format!("item-icon-{}-{}", item.type_name, item.id))
                .label(format!("{} icon", type_label))
                .color(t.text_secondary),
        )
        .child(
            col()
                .grow(1.0)
                .gap(2.0)
                .child(
                    row()
                        .w_full()
                        .gap(6.0)
                        .center_y()
                        .child(
                            text(item.name.clone())
                                .key("item-name")
                                .font(ROW_TITLE)
                                .color(t.text)
                                .grow(1.0)
                                .truncate(),
                        )
                        .child(text(star).font(ROW_TITLE).color(accent(dark))),
                )
                .child(
                    text(if item.subtitle.is_empty() {
                        type_label.as_ref()
                    } else {
                        item.subtitle.as_str()
                    })
                    .font(ROW_SUBTITLE)
                    .color(t.text_secondary)
                    .truncate(),
                ),
        )
}

fn details(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let Some(item) = s.selected_item() else {
        return col()
            .key("details")
            .grow(1.0)
            .h_full()
            .center()
            .gap(6.0)
            .fill(t.bg)
            .child(text("No item selected").font(WINDOW_TITLE).color(t.text))
            .child(
                text("Choose an item from the list to see its details.")
                    .font(ROW_SUBTITLE)
                    .color(t.text_secondary),
            );
    };

    let favorite = item.favorite;
    let type_label = icons::label(&item.type_name);
    let mut fields = col()
        .key("detail-fields")
        .padding(20.0)
        .gap(14.0)
        .child(field("Type", type_label.as_ref(), dark));
    if let Some(details) = s.details.as_ref().filter(|details| details.id == item.id) {
        for detail in &details.fields {
            let revealed = s.revealed.as_deref() == Some(detail.key.as_str());
            let sensitive = matches!(detail.value, FieldValue::Masked);
            let shown = match &detail.value {
                FieldValue::Text(value) => value.clone(),
                FieldValue::Masked if revealed => {
                    revealed_value(&detail.key).unwrap_or_else(|| "••••••••".to_owned())
                }
                FieldValue::Masked => "••••••••".to_owned(),
            };
            let shown = if detail.key == "totp" {
                s.totp
                    .as_ref()
                    .filter(|meta| meta.item_id == item.id)
                    .map(|meta| {
                        let code = totp_value(&meta.item_id).unwrap_or_else(|| "······".to_owned());
                        format!("{code} · {}s", meta.ttl)
                    })
                    .unwrap_or(shown)
            } else {
                shown
            };
            let key = detail.key.clone();
            let label = detail.label.clone();
            let copy_key = key.clone();
            let copy_label = label.clone();
            let value = field(&label, &shown, dark)
                .grow(1.0)
                .key(format!("detail-field-{key}"))
                .role("button")
                .label(format!("Copy {label}"))
                .help(format!("Copy {label}"))
                .cursor(Cursor::Pointer)
                .hover(t.hover)
                .press(t.press)
                .on_press(move |state| copy_field(state, &copy_key, &copy_label, sensitive));
            let mut buttons = row().gap(6.0);
            if sensitive {
                let reveal_key = key.clone();
                let text = if revealed { "Hide" } else { "Reveal" };
                buttons = buttons.child(action(
                    &format!("reveal-{reveal_key}"),
                    text,
                    dark,
                    move |state| toggle_reveal(state, reveal_key.clone()),
                ));
            }
            if key.starts_with("uri-") {
                if let FieldValue::Text(url) = &detail.value {
                    let url = url.clone();
                    buttons = buttons.child(action_tx(
                        &format!("open-{key}"),
                        "Open",
                        dark,
                        move |state, tx| {
                            if let Err(error) = tx.open_url(&url) {
                                state.notice = Some(format!("Cannot open website: {error}"));
                            }
                        },
                    ));
                }
            }
            fields = fields.child(row().gap(10.0).center_y().child(value).child(buttons));
        }
    } else if !item.subtitle.is_empty() {
        fields = fields.child(field("Username", &item.subtitle, dark));
    }
    fields = fields.child(field(
        "Folder",
        if item.folder.is_empty() {
            "—"
        } else {
            &item.folder
        },
        dark,
    ));
    if !s
        .details
        .as_ref()
        .is_some_and(|details| details.id == item.id)
    {
        fields = fields.child(field(
            "Updated",
            if item.updated.is_empty() {
                "—"
            } else {
                &item.updated
            },
            dark,
        ));
    }
    if let Some(details) = s.details.as_ref().filter(|details| details.id == item.id) {
        for attachment in &details.attachments {
            let label = if attachment.size.is_empty() {
                attachment.filename.clone()
            } else {
                format!("{} · {}", attachment.filename, attachment.size)
            };
            let value = field("Attachment", &label, dark)
                .grow(1.0)
                .key(format!("attachment-field-{}", attachment.id));
            let id = attachment.id.clone();
            let filename = attachment.filename.clone();
            let preview_id = id.clone();
            let preview_filename = filename.clone();
            let download_id = id.clone();
            let download_filename = filename.clone();
            let buttons = row()
                .gap(6.0)
                .child(action(
                    &format!("preview-attachment-{id}"),
                    "Preview",
                    dark,
                    move |state| {
                        open_attachment(state, preview_id.clone(), preview_filename.clone(), true)
                    },
                ))
                .child(action(
                    &format!("download-attachment-{id}"),
                    "Download",
                    dark,
                    move |state| {
                        open_attachment(
                            state,
                            download_id.clone(),
                            download_filename.clone(),
                            false,
                        )
                    },
                ));
            fields = fields.child(row().gap(10.0).center_y().child(value).child(buttons));
        }
    }

    let in_trash = s.location == Location::Trash;
    fields = fields.child(
        row()
            .key("detail-actions")
            .gap(8.0)
            .child(action_tx("edit-item", "Edit", dark, |state, tx| {
                start_edit(state);
                tx.focus("edit-name", true);
            }))
            .child(if in_trash {
                action("restore-item", "Restore", dark, restore_item)
            } else {
                el()
            })
            .child(action(
                "delete-item",
                if s.pending_delete.as_deref() == Some(item.id.as_str()) {
                    "Confirm delete"
                } else if in_trash {
                    "Delete forever"
                } else {
                    "Delete"
                },
                dark,
                move |state| delete_item(state, in_trash),
            )),
    );

    col()
        .key("details")
        .grow(1.0)
        .h_full()
        .fill(t.bg)
        .child(
            row()
                .key("detail-header")
                .w_full()
                .h(64.0)
                .px(20.0)
                .gap(12.0)
                .center_y()
                .child(
                    icon(icons::for_type(&item.type_name), 22.0)
                        .key(format!("detail-icon-{}", item.type_name))
                        .label(format!("{} icon", type_label))
                        .color(t.text_secondary),
                )
                .child(
                    col()
                        .gap(2.0)
                        .child(
                            text(item.name.clone())
                                .key("detail-name")
                                .font(WINDOW_TITLE)
                                .color(t.text),
                        )
                        .child(text(type_label.as_ref()).font(META).color(t.text_secondary)),
                )
                .child(el().grow(1.0))
                .child(action(
                    "favorite-toggle",
                    if favorite { "Unfavorite" } else { "Favorite" },
                    dark,
                    move |state| set_favorite(state, !favorite),
                )),
        )
        .child(el().h(1.0).w_full().fill(t.separator))
        .child(fields)
}

fn field(label: &str, value: &str, dark: bool) -> El {
    let t = tok::theme(dark);
    col()
        .key(format!("field-{label}"))
        .gap(3.0)
        .child(
            text(label.to_uppercase())
                .font(SECTION_TITLE)
                .color(t.text_secondary),
        )
        .child(
            text(value.to_owned())
                .key(format!("field-value-{label}"))
                .font(ROW_SUBTITLE)
                .color(t.text)
                .selectable(true),
        )
}

fn status_bar(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let label = if s.busy {
        "Working…".to_owned()
    } else if let Some(notice) = &s.notice {
        notice.clone()
    } else if s.snapshot.status.is_empty() {
        "Backend unavailable".to_owned()
    } else {
        format!("{} · {} items", s.snapshot.email, s.snapshot.items_count)
    };
    let dot = if s.notice.is_some() || s.snapshot.status.is_empty() {
        t.text_secondary
    } else {
        accent(dark)
    };

    let sync = if s.snapshot.last_sync.is_empty() {
        String::new()
    } else {
        format!(
            " · synced {}",
            s.snapshot.last_sync.chars().take(16).collect::<String>()
        )
    };
    let server = format!("{}{}", s.snapshot.server, sync);

    row()
        .key("status-bar")
        .w_full()
        .h(30.0)
        .px(14.0)
        .gap(8.0)
        .center_y()
        .fill(t.card)
        .child(el().key("status-dot").size(6.0, 6.0).radius(3.0).fill(dot))
        .child(
            text(label)
                .key("status-label")
                .font(META)
                .color(t.text_secondary)
                .grow(1.0)
                .truncate(),
        )
        .child(text(server).font(META).color(t.text_secondary).truncate())
}

fn generator_toggle(
    key: &str,
    label: &str,
    active: bool,
    dark: bool,
    on_press: fn(&mut State),
) -> El {
    action(
        key,
        if active {
            format!("✓ {label}")
        } else {
            label.to_owned()
        },
        dark,
        on_press,
    )
}

fn generator(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let Some(g) = s.generator.as_ref() else {
        return el();
    };
    let header = text("Generate a strong value with the Kurama daemon.")
        .font(ROW_SUBTITLE)
        .color(t.text_secondary);

    let modes = row()
        .key("generator-modes")
        .gap(12.0)
        .child(action(
            "generator-mode-password",
            if g.passphrase {
                "Password"
            } else {
                "• Password"
            },
            dark,
            |state| set_generator_passphrase(state, false),
        ))
        .child(action(
            "generator-mode-passphrase",
            if g.passphrase {
                "• Passphrase"
            } else {
                "Passphrase"
            },
            dark,
            |state| set_generator_passphrase(state, true),
        ));

    let mut body = col().child(modes);
    if g.passphrase {
        body = body
            .child(
                row()
                    .key("generator-words")
                    .gap(12.0)
                    .child(action("generator-words-decrease", "−", dark, |state| {
                        adjust_generator_words(state, -1)
                    }))
                    .child(
                        text(format!("Words: {}", g.words))
                            .key("generator-words-value")
                            .font(ROW_TITLE)
                            .color(t.text),
                    )
                    .child(action("generator-words-increase", "+", dark, |state| {
                        adjust_generator_words(state, 1)
                    })),
            )
            .child(form_field(
                "generator-separator",
                "Separator",
                "-",
                &g.separator,
                dark,
                |state| &mut state.generator.as_mut().unwrap().separator,
                generate_password,
            ))
            .child(generator_toggle(
                "generator-capitalize",
                "Capitalize words",
                g.capitalize,
                dark,
                |state| toggle_generator_option(state, GeneratorOption::Capitalize),
            ));
    } else {
        body = body
            .child(
                row()
                    .key("generator-length")
                    .gap(12.0)
                    .child(action("generator-length-decrease", "−", dark, |state| {
                        adjust_generator_length(state, -1)
                    }))
                    .child(
                        text(format!("Length: {}", g.length))
                            .key("generator-length-value")
                            .font(ROW_TITLE)
                            .color(t.text),
                    )
                    .child(action("generator-length-increase", "+", dark, |state| {
                        adjust_generator_length(state, 1)
                    })),
            )
            .child(
                row()
                    .key("generator-options")
                    .gap(8.0)
                    .child(generator_toggle(
                        "generator-uppercase",
                        "A-Z",
                        g.uppercase,
                        dark,
                        |state| toggle_generator_option(state, GeneratorOption::Uppercase),
                    ))
                    .child(generator_toggle(
                        "generator-lowercase",
                        "a-z",
                        g.lowercase,
                        dark,
                        |state| toggle_generator_option(state, GeneratorOption::Lowercase),
                    ))
                    .child(generator_toggle(
                        "generator-numbers",
                        "0-9",
                        g.numbers,
                        dark,
                        |state| toggle_generator_option(state, GeneratorOption::Numbers),
                    ))
                    .child(generator_toggle(
                        "generator-special",
                        "!@#",
                        g.special,
                        dark,
                        |state| toggle_generator_option(state, GeneratorOption::Special),
                    ))
                    .child(generator_toggle(
                        "generator-avoid-ambiguous",
                        "Avoid ambiguous",
                        g.avoid_ambiguous,
                        dark,
                        |state| toggle_generator_option(state, GeneratorOption::AvoidAmbiguous),
                    )),
            );
    }

    body = body.child(action(
        "generator-generate",
        "Generate",
        dark,
        generate_password,
    ));
    if !g.value.text.is_empty() {
        let result = text_input(&g.value, type_::BODY)
            .key("generator-value")
            .label("Generated value")
            .secure()
            .revealed(true)
            .color(t.text)
            .w(480.0)
            .h(36.0)
            .padding(10.0)
            .radius(8.0)
            .fill(t.card)
            .border(1.0, t.card_border)
            .on_edit(|state: &mut State| &mut state.generator.as_mut().unwrap().value);
        body = body
            .child(
                text("Generated value")
                    .font(SECTION_TITLE)
                    .color(t.text_secondary),
            )
            .child(result)
            .child(
                text(format!("{:.1} bits of entropy", g.entropy))
                    .key("generator-entropy")
                    .font(META)
                    .color(t.text_secondary),
            );
    }

    let mut footer = row()
        .gap(12.0)
        .child(action("generator-copy", "Copy", dark, copy_generated));
    if g.target != GeneratorTarget::None {
        footer = footer.child(action("generator-use", "Use in form", dark, use_generated));
    }
    footer = footer.child(action("generator-close", "Close", dark, close_generator));

    form_panel(
        "generator-form",
        "Password generator",
        dark,
        header,
        body,
        footer,
        s.form_error.as_deref(),
    )
}

fn account(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let Some(auth) = s.auth.as_ref() else {
        return el();
    };
    let title = match &auth.mode {
        AuthMode::Password | AuthMode::ApiKey => "Connect account",
        AuthMode::TwoFactor { .. } => "Two-factor verification",
        AuthMode::NewDevice => "Verify new device",
        AuthMode::Fido2 { .. } => "Security key verification",
    };
    let header = text("Connect Kurama to your vault server.")
        .font(ROW_SUBTITLE)
        .color(t.text_secondary);

    let modes = row()
        .key("account-modes")
        .gap(12.0)
        .child(action(
            "account-mode-password",
            if auth.mode == AuthMode::Password {
                "• Master password"
            } else {
                "Master password"
            },
            dark,
            |state| set_auth_mode(state, AuthMode::Password),
        ))
        .child(action(
            "account-mode-api",
            if auth.mode == AuthMode::ApiKey {
                "• API key"
            } else {
                "API key"
            },
            dark,
            |state| set_auth_mode(state, AuthMode::ApiKey),
        ))
        .child(action(
            "account-security-key",
            "Security key",
            dark,
            submit_fido2,
        ));

    let mut body = col()
        .child(form_field(
            "account-server",
            "Server URL",
            "https://vault.example.com",
            &auth.server,
            dark,
            |state| &mut state.auth.as_mut().unwrap().server,
            submit_auth,
        ))
        .child(modes);
    match &auth.mode {
        AuthMode::Password => {
            body = body
                .child(form_field(
                    "account-email",
                    "Email",
                    "user@example.com",
                    &auth.email,
                    dark,
                    |state| &mut state.auth.as_mut().unwrap().email,
                    submit_auth,
                ))
                .child(form_field(
                    "account-password",
                    "Master password",
                    "Master password",
                    &auth.password,
                    dark,
                    |state| &mut state.auth.as_mut().unwrap().password,
                    submit_auth,
                ));
        }
        AuthMode::ApiKey => {
            body = body
                .child(form_field(
                    "account-client-id",
                    "Client ID",
                    "user.id",
                    &auth.client_id,
                    dark,
                    |state| &mut state.auth.as_mut().unwrap().client_id,
                    submit_auth,
                ))
                .child(form_field(
                    "account-client-secret",
                    "Client secret",
                    "Client secret",
                    &auth.client_secret,
                    dark,
                    |state| &mut state.auth.as_mut().unwrap().client_secret,
                    submit_auth,
                ));
        }
        AuthMode::TwoFactor { provider, .. } => {
            let method = match provider {
                Some(0) => "Authenticator app",
                Some(1) => "Email",
                Some(2) => "Duo",
                Some(3) => "YubiKey",
                Some(4) => "Legacy U2F",
                Some(7) => "FIDO2 WebAuthn",
                _ => "Two-factor provider",
            };
            body = body
                .child(
                    text(format!("Method: {method}"))
                        .key("account-provider")
                        .font(ROW_SUBTITLE)
                        .color(t.text_secondary),
                )
                .child(form_field(
                    "account-code",
                    "Verification code",
                    "123456",
                    &auth.code,
                    dark,
                    |state| &mut state.auth.as_mut().unwrap().code,
                    submit_auth,
                ))
                .child(action(
                    "account-send-email",
                    "Send code by email",
                    dark,
                    send_two_factor_email,
                ));
        }
        AuthMode::NewDevice => {
            body = body.child(form_field(
                "account-new-device",
                "Verification code",
                "Code from your email",
                &auth.new_device_code,
                dark,
                |state| &mut state.auth.as_mut().unwrap().new_device_code,
                submit_auth,
            ));
        }
        AuthMode::Fido2 { status, .. } => {
            let detail = status.as_deref().unwrap_or(
                "Insert or tap your security key, then continue when your browser prompts you.",
            );
            body = body
                .child(
                    text(detail.to_owned())
                        .key("account-fido2-status")
                        .font(ROW_SUBTITLE)
                        .color(t.text_secondary),
                )
                .child(action(
                    "account-fido2-continue",
                    "Continue with security key",
                    dark,
                    submit_fido2,
                ));
        }
    }

    let mut footer = row()
        .gap(12.0)
        .child(action("account-submit", "Connect", dark, submit_auth));
    if matches!(
        &auth.mode,
        AuthMode::TwoFactor { .. } | AuthMode::NewDevice | AuthMode::Fido2 { .. }
    ) {
        footer = footer.child(action("account-back", "Back", dark, |state| {
            set_auth_mode(state, AuthMode::Password)
        }));
    }

    form_panel(
        "account-form",
        title,
        dark,
        header,
        body,
        footer,
        s.form_error.as_deref().or(s.notice.as_deref()),
    )
}

fn unlock(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let notice = s
        .notice
        .as_deref()
        .unwrap_or("Enter your master password to unlock this vault.");

    let password = text_input(&s.password, type_::BODY)
        .key("unlock-password")
        .label("Master password")
        .placeholder("Master password", t.text_secondary)
        .secure()
        .single_line()
        .color(t.text)
        .w(320.0)
        .h(36.0)
        .padding(10.0)
        .radius(8.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .on_edit(|s: &mut State| &mut s.password)
        .on_submit(submit_unlock);

    let mut actions = row().key("unlock-actions").gap(8.0).child(action_tx(
        "unlock-button",
        "Unlock",
        dark,
        submit_unlock,
    ));
    if s.snapshot.quick_unlock_ready {
        actions = actions.child(action(
            "quick-unlock-button",
            "Quick Unlock",
            dark,
            submit_quick_unlock,
        ));
    }

    col()
        .key("unlock")
        .w_full()
        .grow(1.0)
        .center()
        .gap(14.0)
        .fill(t.bg)
        .child(
            text("Vault locked")
                .key("placeholder-title")
                .font(WINDOW_TITLE)
                .color(t.text),
        )
        .child(password)
        .child(actions)
        .child(
            text(notice.to_owned())
                .key("unlock-notice")
                .font(ROW_SUBTITLE)
                .color(t.text_secondary),
        )
}

fn placeholder(
    s: &State,
    dark: bool,
    title: &str,
    body: &str,
    button_label: &str,
    request: Request,
) -> El {
    let t = tok::theme(dark);
    let notice = s.notice.as_deref().unwrap_or(body);
    col()
        .key("placeholder")
        .w_full()
        .grow(1.0)
        .center()
        .gap(10.0)
        .fill(t.bg)
        .child(
            text(title)
                .key("placeholder-title")
                .font(WINDOW_TITLE)
                .color(t.text),
        )
        .child(
            text(notice.to_owned())
                .key("placeholder-body")
                .font(ROW_SUBTITLE)
                .color(t.text_secondary),
        )
        .child(action("placeholder-action", button_label, dark, move |s| {
            dispatch(s, request.clone())
        }))
}

fn action(
    key: &str,
    label: impl Into<String>,
    dark: bool,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    action_tx(key, label, dark, move |s, _tx| on_press(s))
}

fn action_tx(
    key: &str,
    label: impl Into<String>,
    dark: bool,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    let t = tok::theme(dark);
    let label = label.into();
    el().key(key)
        .role("button")
        .label(label.clone())
        .focusable(true)
        .h(32.0)
        .px(12.0)
        .center()
        .radius(8.0)
        .fill(t.control_track)
        .hover(t.hover)
        .press(t.press)
        .cursor(Cursor::Pointer)
        .on_press_tx(on_press)
        .child(text(label).font(type_::LABEL).color(t.text))
}
