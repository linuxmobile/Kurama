//! Item-type glyphs used by the vault list and detail header.

use std::borrow::Cow;

use rune::Icon;

static LOGIN: Icon = Icon {
    name: "kurama-login",
    view: 16.0,
    body: r#"<circle cx="5.25" cy="5.25" r="2.75" fill="none" stroke="currentColor" stroke-width="1.25"/><path d="M7.25 7.25L13 13M10.25 10.25L12 8.5M11.75 11.75L13.5 10" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static NOTE: Icon = Icon {
    name: "kurama-note",
    view: 16.0,
    body: r#"<path d="M3.5 2.5H9.5L12.5 5.5V13.5H3.5V2.5Z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/><path d="M9.5 2.5V5.5H12.5M5.75 8H10.25M5.75 10.5H10.25" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#,
};

static CARD: Icon = Icon {
    name: "kurama-card",
    view: 16.0,
    body: r#"<rect x="2" y="3.5" width="12" height="9" rx="1.75" fill="none" stroke="currentColor" stroke-width="1.25"/><path d="M2 6.5H14M4.5 10H7" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#,
};

static IDENTITY: Icon = Icon {
    name: "kurama-identity",
    view: 16.0,
    body: r#"<circle cx="8" cy="8" r="6.25" fill="none" stroke="currentColor" stroke-width="1.25"/><circle cx="8" cy="6" r="1.75" fill="none" stroke="currentColor" stroke-width="1.25"/><path d="M4.5 12C5 10.25 6.35 9.5 8 9.5C9.65 9.5 11 10.25 11.5 12" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#,
};

static SSH_KEY: Icon = Icon {
    name: "kurama-ssh-key",
    view: 16.0,
    body: r#"<rect x="2" y="3" width="12" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.25"/><path d="M4.75 6.25L6.5 8L4.75 9.75M7.5 9.75H11" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ITEM: Icon = Icon {
    name: "kurama-item",
    view: 16.0,
    body: r#"<rect x="3" y="3" width="10" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.25"/>"#,
};

static ACTION_NEW: Icon = Icon {
    name: "kurama-action-new",
    view: 24.0,
    body: r#"<path d="M5 12h14M12 5v14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_SUN: Icon = Icon {
    name: "kurama-action-sun",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="4" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};

static ACTION_MOON: Icon = Icon {
    name: "kurama-action-moon",
    view: 24.0,
    body: r#"<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_SENDS: Icon = Icon {
    name: "kurama-action-sends",
    view: 24.0,
    body: r#"<path d="m22 2-7 20-4-9-9-4Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M22 2 11 13" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_SETTINGS: Icon = Icon {
    name: "kurama-action-settings",
    view: 24.0,
    body: r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="3" fill="none" stroke="currentColor" stroke-width="1.6"/>"#,
};

static ACTION_SSH: Icon = Icon {
    name: "kurama-action-ssh",
    view: 24.0,
    body: r#"<circle cx="8" cy="15" r="4" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="m10.8 12.2 8.2-8.2M17 3l4 4M15 5l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_SYNC: Icon = Icon {
    name: "kurama-action-sync",
    view: 24.0,
    body: r#"<path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8M3 3v5h5M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16M16 16h5v5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_GENERATE: Icon = Icon {
    name: "kurama-action-generate",
    view: 24.0,
    body: r#"<path d="m12 3 1.9 5.8L20 11l-6.1 2.2L12 19l-1.9-5.8L4 11l6.1-2.2ZM19 3v4M21 5h-4M5 17v4M7 19H3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_HELP: Icon = Icon {
    name: "kurama-action-help",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3M12 17h.01" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_LOCK: Icon = Icon {
    name: "kurama-action-lock",
    view: 24.0,
    body: r#"<rect x="3" y="11" width="18" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M7 11V7a5 5 0 0 1 10 0v4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};

static ACTION_LOGOUT: Icon = Icon {
    name: "kurama-action-logout",
    view: 24.0,
    body: r#"<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_VAULT: Icon = Icon {
    name: "kurama-action-vault",
    view: 24.0,
    body: r#"<path d="M12 2.5 4.5 5.5v6c0 4.8 3.1 8.5 7.5 10 4.4-1.5 7.5-5.2 7.5-10v-6Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/><path d="M9 12.3l2 2 4-4.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_USER: Icon = Icon {
    name: "kurama-action-user",
    view: 24.0,
    body: r#"<circle cx="12" cy="8" r="3.5" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M5 20c0-3.8 3.1-6.5 7-6.5s7 2.7 7 6.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>"#,
};

static ACTION_PANEL: Icon = Icon {
    name: "kurama-action-panel",
    view: 24.0,
    body: r#"<rect x="3" y="4.5" width="18" height="15" rx="2" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M9.5 4.5v15" fill="none" stroke="currentColor" stroke-width="1.8"/>"#,
};

static ACTION_SEARCH: Icon = Icon {
    name: "kurama-action-search",
    view: 24.0,
    body: r#"<circle cx="10.8" cy="10.8" r="6.3" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="m15.5 15.5 5 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};

static ACTION_CHEVRON_LEFT: Icon = Icon {
    name: "kurama-action-chevron-left",
    view: 24.0,
    body: r#"<path d="m14.5 5-7 7 7 7" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_CHEVRON_RIGHT: Icon = Icon {
    name: "kurama-action-chevron-right",
    view: 24.0,
    body: r#"<path d="m9.5 5 7 7-7 7" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_STAR: Icon = Icon {
    name: "kurama-action-star",
    view: 24.0,
    body: r#"<path d="m12 3.2 2.7 5.6 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1-4.4-4.3 6.1-.9Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/>"#,
};

static ACTION_STAR_FILLED: Icon = Icon {
    name: "kurama-action-star-filled",
    view: 24.0,
    body: r#"<path d="m12 3.2 2.7 5.6 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1-4.4-4.3 6.1-.9Z" fill="currentColor"/>"#,
};

static ACTION_COPY: Icon = Icon {
    name: "kurama-action-copy",
    view: 24.0,
    body: r#"<rect x="8" y="8" width="12" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" fill="none" stroke="currentColor" stroke-width="1.8"/>"#,
};

static ACTION_EDIT: Icon = Icon {
    name: "kurama-action-edit",
    view: 24.0,
    body: r#"<path d="m4 16.5-.6 4 4-.6L19 8.3 15.7 5 4 16.5Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><path d="m13.8 6.9 3.3 3.3" fill="none" stroke="currentColor" stroke-width="1.8"/>"#,
};

static ACTION_EXTERNAL: Icon = Icon {
    name: "kurama-action-external",
    view: 24.0,
    body: r#"<path d="M14 4h6v6M20 4l-8.5 8.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M19 14.5V19a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 19V6.5A1.5 1.5 0 0 1 5 5h4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};

static ACTION_GLOBE: Icon = Icon {
    name: "kurama-action-globe",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M3.7 12h16.6M12 3.5c2.4 2.3 3.6 5.1 3.6 8.5S14.4 18.2 12 20.5c-2.4-2.3-3.6-5.1-3.6-8.5S9.6 5.8 12 3.5Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>"#,
};

static ACTION_EYE: Icon = Icon {
    name: "kurama-action-eye",
    view: 24.0,
    body: r#"<path d="M2.5 12S6 5.8 12 5.8 21.5 12 21.5 12 18 18.2 12 18.2 2.5 12 2.5 12Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><circle cx="12" cy="12" r="2.8" fill="none" stroke="currentColor" stroke-width="1.8"/>"#,
};

static ACTION_EYE_OFF: Icon = Icon {
    name: "kurama-action-eye-off",
    view: 24.0,
    body: r#"<path d="M4 4l16 16M9.7 6.2A9.7 9.7 0 0 1 12 6c6 0 9.5 6 9.5 6a16 16 0 0 1-3.2 3.7M6.2 8.2A16 16 0 0 0 2.5 12S6 18 12 18a9.4 9.4 0 0 0 3-.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_FOLDER: Icon = Icon {
    name: "kurama-action-folder",
    view: 24.0,
    body: r#"<path d="M3 7a2 2 0 0 1 2-2h4.4l2 2.4H19a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>"#,
};

static ACTION_PLUS: Icon = Icon {
    name: "kurama-action-plus",
    view: 24.0,
    body: r#"<path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

static ACTION_TRASH: Icon = Icon {
    name: "kurama-action-trash",
    view: 24.0,
    body: r#"<path d="M4 7h16M9 7V4.5h6V7M6 7l1 13h10l1-13M10 10.5v6M14 10.5v6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_GRID: Icon = Icon {
    name: "kurama-action-grid",
    view: 24.0,
    body: r#"<rect x="3.5" y="3.5" width="7" height="7" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.7"/>"#,
};

static ACTION_KEY: Icon = Icon {
    name: "kurama-action-key",
    view: 24.0,
    body: r#"<circle cx="8" cy="9" r="4.2" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="m11 12 9 9M16.5 16.5l2-2M14 14l2-2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};

static ACTION_SHIELD: Icon = Icon {
    name: "kurama-action-shield",
    view: 24.0,
    body: r#"<path d="M12 2.5 4.5 5.5v6c0 4.8 3.1 8.5 7.5 10 4.4-1.5 7.5-5.2 7.5-10v-6Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><path d="M12 8v5M12 16h.01" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>"#,
};

static ACTION_COLLECTION: Icon = Icon {
    name: "kurama-action-collection",
    view: 24.0,
    body: r#"<path d="M4 20V10M9.3 20V4M14.7 20v-7M20 20V7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>"#,
};

static ACTION_ATTACH: Icon = Icon {
    name: "kurama-action-attach",
    view: 24.0,
    body: r#"<path d="M20 11.5 12.6 19a4.6 4.6 0 0 1-6.5-6.5l8.2-8.2a3 3 0 0 1 4.2 4.2l-8.1 8.2a1.4 1.4 0 0 1-2-2l7.3-7.3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_CLOCK: Icon = Icon {
    name: "kurama-action-clock",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M12 7.5V12l3 2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

static ACTION_FILTER: Icon = Icon {
    name: "kurama-action-filter",
    view: 24.0,
    body: r##"<path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><circle cx="9" cy="7" r="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="15" cy="17" r="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/>"##,
};

pub fn action(name: &str) -> &'static Icon {
    match name {
        "new" => &ACTION_NEW,
        "sun" | "appearance" => &ACTION_SUN,
        "moon" => &ACTION_MOON,
        "sends" => &ACTION_SENDS,
        "settings" => &ACTION_SETTINGS,
        "ssh" => &ACTION_SSH,
        "sync" => &ACTION_SYNC,
        "generate" => &ACTION_GENERATE,
        "help" => &ACTION_HELP,
        "lock" => &ACTION_LOCK,
        "logout" => &ACTION_LOGOUT,
        "vault" => &ACTION_VAULT,
        "user" => &ACTION_USER,
        "card" => &CARD,
        "note" => &NOTE,
        "login" => &LOGIN,
        "panel" => &ACTION_PANEL,
        "search" => &ACTION_SEARCH,
        "chevron-left" => &ACTION_CHEVRON_LEFT,
        "chevron-right" => &ACTION_CHEVRON_RIGHT,
        "star" => &ACTION_STAR,
        "star-filled" => &ACTION_STAR_FILLED,
        "copy" => &ACTION_COPY,
        "edit" => &ACTION_EDIT,
        "external" => &ACTION_EXTERNAL,
        "globe" => &ACTION_GLOBE,
        "eye" => &ACTION_EYE,
        "eye-off" => &ACTION_EYE_OFF,
        "folder" => &ACTION_FOLDER,
        "plus" => &ACTION_PLUS,
        "trash" => &ACTION_TRASH,
        "grid" => &ACTION_GRID,
        "key" => &ACTION_KEY,
        "shield" => &ACTION_SHIELD,
        "collection" => &ACTION_COLLECTION,
        "attach" => &ACTION_ATTACH,
        "filter" => &ACTION_FILTER,
        "clock" => &ACTION_CLOCK,
        _ => &ITEM,
    }
}

pub fn for_type(type_name: &str) -> &'static Icon {
    match type_name {
        "login" => &LOGIN,
        "note" => &NOTE,
        "card" => &CARD,
        "identity" => &IDENTITY,
        "ssh_key" => &SSH_KEY,
        _ => &ITEM,
    }
}

pub fn label(type_name: &str) -> Cow<'_, str> {
    match type_name {
        "login" => Cow::Borrowed("Login"),
        "note" => Cow::Borrowed("Secure note"),
        "card" => Cow::Borrowed("Card"),
        "identity" => Cow::Borrowed("Identity"),
        "ssh_key" => Cow::Borrowed("SSH key"),
        "" => Cow::Borrowed("Item"),
        other => Cow::Borrowed(other),
    }
}

#[cfg(test)]
mod tests {
    use super::{for_type, label};

    #[test]
    fn known_types_have_a_label_and_icon() {
        assert_eq!(label("login"), "Login");
        assert_eq!(label("ssh_key"), "SSH key");
        assert_eq!(label("custom"), "custom");
        assert_eq!(for_type("note").name, "kurama-note");
        assert_eq!(for_type("custom").name, "kurama-item");
    }
}
