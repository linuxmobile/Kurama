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
