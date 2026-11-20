use rune::tok::{self, type_};
use rune::{col, el, icon, row, text, text_input, Color, Cx, El, GrowthSpec, ScrollSpec, Tx};

use crate::backend::{Request, VaultRow};
use crate::icons;
use crate::state::{
    dispatch, open_trash, select_category, select_folder, submit_quick_unlock, submit_search,
    submit_unlock, Category, Location, State,
};
use crate::theme::{
    accent, selected_row, META, ROW_SUBTITLE, ROW_TITLE, SECTION_TITLE, WINDOW_TITLE,
};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.dark(cx.system_dark());
    let t = tok::theme(dark);
    let connected = !s.snapshot.status.is_empty();
    let unlocked = s.snapshot.status == "unlocked";

    let content = if !connected {
        placeholder(
            s,
            dark,
            "Vault not connected",
            "Start the Kurama backend daemon to browse your vault.",
            "Retry",
            Request::Refresh,
        )
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

fn top_bar(s: &State, dark: bool) -> El {
    let t = tok::theme(dark);
    let search = text_input(&s.query, type_::BODY)
        .key("search")
        .label("Search vault")
        .placeholder("Search vault", t.text_secondary)
        .color(t.text)
        .h(34.0)
        .w(320.0)
        .padding(10.0)
        .radius(8.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .on_edit(|s: &mut State| &mut s.query)
        .on_submit(submit_search);

    let brand = col()
        .gap(1.0)
        .child(text("Kurama").key("brand").font(WINDOW_TITLE).color(t.text))
        .child(text("VAULT").font(SECTION_TITLE).color(t.text_secondary));

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
        .child(action(
            "theme",
            if dark { "Light" } else { "Dark" },
            dark,
            move |s| {
                s.dark_override = Some(!dark);
            },
        ))
        .child(action("sync", "Sync", dark, |s| dispatch(s, Request::Sync)))
        .child(action("lock", "Lock", dark, |s| dispatch(s, Request::Lock)))
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

    nav.child(el().grow(1.0)).child(
        text("Kurama · Rune")
            .font(META)
            .color(t.text_secondary)
            .padding(8.0),
    )
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
        .on_press(move |s: &mut State| s.selected = Some(id.clone()))
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

    let username = item.subtitle.clone();
    let type_label = icons::label(&item.type_name);
    let fields = col()
        .key("detail-fields")
        .padding(20.0)
        .gap(14.0)
        .child(field("Type", type_label.as_ref(), dark))
        .child(field(
            "Username",
            if username.is_empty() {
                "—"
            } else {
                &username
            },
            dark,
        ))
        .child(field(
            "Folder",
            if item.folder.is_empty() {
                "—"
            } else {
                &item.folder
            },
            dark,
        ))
        .child(field(
            "Updated",
            if item.updated.is_empty() {
                "—"
            } else {
                &item.updated
            },
            dark,
        ));

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
                .child(if username.is_empty() {
                    el()
                } else {
                    action_tx("copy-user", "Copy username", dark, move |_s, tx| {
                        tx.copy(username.clone())
                    })
                }),
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

fn action(key: &str, label: &str, dark: bool, on_press: impl Fn(&mut State) + 'static) -> El {
    action_tx(key, label, dark, move |s, _tx| on_press(s))
}

fn action_tx(
    key: &str,
    label: &str,
    dark: bool,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    let t = tok::theme(dark);
    el().key(key)
        .role("button")
        .label(label)
        .focusable(true)
        .h(32.0)
        .px(12.0)
        .center()
        .radius(8.0)
        .fill(t.control_track)
        .on_press_tx(on_press)
        .child(text(label.to_owned()).font(type_::LABEL).color(t.text))
}
