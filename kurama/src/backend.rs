//! Bridge between Kurama and the `kurama-host` backend.
//!
//! All backend calls happen on a worker thread through the daemon IPC.
//! State only holds metadata and field keys. A revealed value lives in a
//! dedicated store so it never appears in `State` or its `Debug` output.

use std::fmt;
use std::sync::Mutex;

use serde_json::{json, Value};

/// Timing data for a TOTP code. The code itself stays in a dedicated store.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TotpMeta {
    pub item_id: String,
    pub ttl: u64,
    pub period: u64,
}

/// Non-secret information shown in the item list and detail pane.
#[derive(Clone, Debug, Default)]
pub struct VaultRow {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub type_name: String,
    pub favorite: bool,
    pub folder: String,
    pub updated: String,
}

/// Value of a detail field. Sensitive values never leave the daemon.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FieldValue {
    Text(String),
    Masked,
}

/// Non-secret field shown in the item inspector.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ItemField {
    pub key: String,
    pub label: String,
    pub value: FieldValue,
}

/// Non-secret attachment metadata shown in the item inspector.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AttachmentMeta {
    pub id: String,
    pub filename: String,
    pub size: String,
}

/// Metadata-only view of one decrypted vault item.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct ItemDetails {
    pub id: String,
    pub fields: Vec<ItemField>,
    pub attachments: Vec<AttachmentMeta>,
}

/// Non-secret view of the daemon's `status` response.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub status: String,
    pub email: String,
    pub server: String,
    pub last_sync: String,
    pub items: Vec<VaultRow>,
    pub items_count: usize,
    pub quick_unlock_ready: bool,
}

/// Non-secret status or error notice from the backend event log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice {
    pub kind: String,
    pub message: String,
    pub approval_id: Option<u64>,
}

/// Non-secret view of a pending SSH approval prompt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Approval {
    pub id: u64,
    pub title: String,
    pub action: String,
    pub key_name: String,
    pub client: String,
}

#[derive(Clone, Debug)]
pub enum Request {
    Refresh,
    Trash,
    Details {
        id: String,
    },
    RevealField {
        item_id: String,
        key: String,
    },
    CopyField {
        item_id: String,
        key: String,
        label: String,
        sensitive: bool,
    },
    Totp {
        item_id: String,
    },
    Attachment {
        item_id: String,
        attachment_id: String,
        filename: String,
        preview: bool,
    },
    Delete {
        id: String,
        permanent: bool,
    },
    UpdateName {
        id: String,
        name: String,
    },
    CreateItem {
        name: String,
        item_type: i64,
    },
    Restore {
        id: String,
    },
    SetFavorite {
        id: String,
        favorite: bool,
    },
    Search(String),
    Sync,
    Lock,
    Unlock {
        password: Secret,
    },
    QuickUnlock,
    Events {
        since: u64,
    },
    Approvals,
}

#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.clear();
    }
}

static REVEALED: Mutex<Option<(String, Secret)>> = Mutex::new(None);

#[cfg(test)]
pub static REVEAL_TEST_LOCK: Mutex<()> = Mutex::new(());

static TOTP: Mutex<Option<(String, String)>> = Mutex::new(None);

pub fn stash_totp(item_id: &str, code: String) {
    if let Ok(mut totp) = TOTP.lock() {
        *totp = Some((item_id.to_owned(), code));
    }
}

pub fn totp_value(item_id: &str) -> Option<String> {
    let totp = TOTP.lock().ok()?;
    let (stored_id, code) = totp.as_ref()?;
    (stored_id == item_id).then(|| code.clone())
}

pub fn clear_totp(item_id: &str) {
    if let Ok(mut totp) = TOTP.lock() {
        if totp
            .as_ref()
            .is_some_and(|(stored_id, _)| stored_id == item_id)
        {
            *totp = None;
        }
    }
}

pub fn stash_revealed(key: &str, value: String) {
    if let Ok(mut revealed) = REVEALED.lock() {
        *revealed = Some((key.to_owned(), Secret::new(value)));
    }
}

pub fn revealed_value(key: &str) -> Option<String> {
    let revealed = REVEALED.lock().ok()?;
    let (stored_key, value) = revealed.as_ref()?;
    (stored_key == key).then(|| value.as_str().to_owned())
}

pub fn clear_revealed() {
    if let Ok(mut revealed) = REVEALED.lock() {
        *revealed = None;
    }
}

#[derive(Debug)]
pub enum Event {
    Snapshot(Snapshot),
    Items(Vec<VaultRow>),
    Trash(Vec<VaultRow>),
    Details(ItemDetails),
    Revealed { item_id: String, key: String },
    AttachmentOpened { filename: String, preview: bool },
    Deleted { id: String, permanent: bool },
    ItemRenamed { id: String, name: String },
    ItemCreated { id: String, name: String },
    Restored { id: String },
    Notice(String),
    Totp(TotpMeta),
    FavoriteChanged { id: String, favorite: bool },
    Notices { cursor: u64, notices: Vec<Notice> },
    Approvals(Vec<Approval>),
    Error(String),
}

const DAEMON_HINT: &str = "Kurama backend is not running. Start it with `kurama daemon`.";

/// Perform a request on the worker thread and return the UI-facing result.
pub fn perform(request: Request) -> Event {
    match request {
        Request::Refresh => match snapshot() {
            Ok(snapshot) => Event::Snapshot(snapshot),
            Err(error) => Event::Error(error),
        },
        Request::Search(query) => match call(json!({ "action": "search", "query": query }))
            .and_then(|response| rows(&response))
        {
            Ok(items) => Event::Items(items),
            Err(error) => Event::Error(error),
        },
        Request::Trash => {
            match call(json!({ "action": "trash" })).and_then(|response| trash_rows(&response)) {
                Ok(items) => Event::Trash(items),
                Err(error) => Event::Error(error),
            }
        }
        Request::Details { id } => match details(&id) {
            Ok(details) => Event::Details(details),
            Err(error) => Event::Error(error),
        },
        Request::RevealField { item_id, key } => match revealed_field(&item_id, &key) {
            Ok(value) => {
                stash_revealed(&key, value);
                Event::Revealed { item_id, key }
            }
            Err(error) => Event::Error(error),
        },
        Request::CopyField {
            item_id,
            key,
            label,
            sensitive,
        } => {
            let value = if key == "totp" {
                totp_response(&item_id).map(|(_, code)| code)
            } else {
                revealed_field(&item_id, &key)
            };
            match value {
                Ok(value) => {
                    let copy = json!({
                        "action": "copy",
                        "text": value,
                        "sensitive": sensitive,
                        "timeout": 30
                    });
                    match call(copy) {
                        Ok(_) => Event::Notice(format!("Copied {label}")),
                        Err(error) => Event::Error(error),
                    }
                }
                Err(error) => Event::Error(error),
            }
        }
        Request::Totp { item_id } => match totp_response(&item_id) {
            Ok((meta, code)) => {
                stash_totp(&item_id, code);
                Event::Totp(meta)
            }
            Err(error) => Event::Error(error),
        },
        Request::Attachment {
            item_id,
            attachment_id,
            filename,
            preview,
        } => match attachment(&item_id, &attachment_id, &filename, preview) {
            Ok(filename) => Event::AttachmentOpened { filename, preview },
            Err(error) => Event::Error(error),
        },
        Request::Delete { id, permanent } => {
            let request = json!({
                "action": "delete_item",
                "ids": [id.clone()],
                "permanent": permanent
            });
            match call(request) {
                Ok(_) => Event::Deleted { id, permanent },
                Err(error) => Event::Error(error),
            }
        }
        Request::UpdateName { id, name } => match call(json!({
            "action": "update_item",
            "id": id.clone(),
            "options": { "name": name.clone() }
        })) {
            Ok(response) if response.get("ok").and_then(Value::as_bool) == Some(true) => {
                Event::ItemRenamed { id, name }
            }
            Ok(response) => Event::Error(text(&response, "error")),
            Err(error) => Event::Error(error),
        },
        Request::CreateItem { name, item_type } => match call(json!({
            "action": "create_item",
            "options": {
                "name": name.clone(),
                "item_type": item_type
            }
        })) {
            Ok(response) if response.get("ok").and_then(Value::as_bool) == Some(true) => {
                let id = response
                    .get("item")
                    .and_then(|item| item.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                Event::ItemCreated { id, name }
            }
            Ok(response) => Event::Error(text(&response, "error")),
            Err(error) => Event::Error(error),
        },
        Request::Restore { id } => match call(json!({
            "action": "restore_item",
            "id": id.clone()
        })) {
            Ok(_) => Event::Restored { id },
            Err(error) => Event::Error(error),
        },
        Request::SetFavorite { id, favorite } => {
            let request = json!({
                "action": "favorite_item",
                "ids": [id.clone()],
                "value": favorite
            });
            match call(request) {
                Ok(_) => Event::FavoriteChanged { id, favorite },
                Err(error) => Event::Error(error),
            }
        }
        Request::Sync => match call(json!({ "action": "sync" })) {
            Ok(_) => match snapshot() {
                Ok(snapshot) => Event::Snapshot(snapshot),
                Err(error) => Event::Error(error),
            },
            Err(error) => Event::Error(error),
        },
        Request::Lock => match call(json!({ "action": "lock" })) {
            Ok(_) => match snapshot() {
                Ok(snapshot) => Event::Snapshot(snapshot),
                Err(error) => Event::Error(error),
            },
            Err(error) => Event::Error(error),
        },
        Request::Unlock { password } => {
            match call(json!({ "action": "unlock", "password": password.as_str() })) {
                Ok(_) => match snapshot() {
                    Ok(snapshot) => Event::Snapshot(snapshot),
                    Err(error) => Event::Error(error),
                },
                Err(_) => Event::Error("Vault unlock failed".to_owned()),
            }
        }
        Request::QuickUnlock => match call(json!({ "action": "quick_unlock" })) {
            Ok(_) => match snapshot() {
                Ok(snapshot) => Event::Snapshot(snapshot),
                Err(error) => Event::Error(error),
            },
            Err(_) => Event::Error("Quick unlock failed".to_owned()),
        },
        Request::Events { since } => match call(json!({ "action": "events", "since": since })) {
            Ok(response) => {
                let (cursor, notices) = notices(&response);
                Event::Notices { cursor, notices }
            }
            Err(_) => Event::Notices {
                cursor: since,
                notices: Vec::new(),
            },
        },
        Request::Approvals => match call(json!({ "action": "ssh_agent_pending" })) {
            Ok(response) => Event::Approvals(approvals(&response)),
            Err(_) => Event::Approvals(Vec::new()),
        },
    }
}

fn call(body: Value) -> Result<Value, String> {
    let response = kurama_host::daemon::send_daemon_request(&body).ok_or(DAEMON_HINT)?;
    if response.get("ok") == Some(&Value::Bool(false)) {
        let message = response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Backend request failed");
        return Err(message.to_owned());
    }
    Ok(response)
}

fn snapshot() -> Result<Snapshot, String> {
    let status = call(json!({ "action": "status" }))?;
    let state = status
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let items = if state == "unlocked" {
        rows(&call(json!({ "action": "list" }))?)?
    } else {
        Vec::new()
    };

    let items_count = items.len();
    let quick_unlock_ready = call(json!({ "action": "ping" }))
        .ok()
        .and_then(|response| response.get("quick_unlock").and_then(Value::as_bool))
        .unwrap_or(false);

    Ok(Snapshot {
        status: state.to_owned(),
        email: text(&status, "user_email"),
        server: text(&status, "server_url"),
        last_sync: text(&status, "last_sync"),
        items,
        items_count,
        quick_unlock_ready,
    })
}

fn rows(response: &Value) -> Result<Vec<VaultRow>, String> {
    let Some(values) = response.as_array() else {
        return Err("Unexpected item list from backend".to_owned());
    };
    Ok(values.iter().filter_map(row).collect())
}

fn trash_rows(response: &Value) -> Result<Vec<VaultRow>, String> {
    let values = response
        .get("items")
        .ok_or("Unexpected Trash response".to_owned())?;
    rows(values)
}

fn row(value: &Value) -> Option<VaultRow> {
    let id = value.get("id")?.as_str()?.to_owned();
    let subtitle = value
        .get("sub_title")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("login")
                .and_then(|login| login.get("username"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();

    Some(VaultRow {
        id,
        name: text(value, "name"),
        subtitle,
        type_name: text(value, "type_name"),
        favorite: value
            .get("favorite")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        folder: text(value, "folder_name"),
        updated: text(value, "updated_at"),
    })
}

fn details(id: &str) -> Result<ItemDetails, String> {
    let response = call(json!({ "action": "get_item", "query": id }))?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(text(&response, "error"));
    }
    let item = response
        .get("item")
        .ok_or_else(|| "Item details are unavailable".to_owned())?;
    Ok(ItemDetails {
        id: id.to_owned(),
        fields: item_fields(item),
        attachments: attachment_metadata(item),
    })
}

fn attachment_metadata(item: &Value) -> Vec<AttachmentMeta> {
    item.get("attachments")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    let id = value.get("id")?.as_str()?;
                    let filename = first_text(value, &["fileName", "filename", "name"]);
                    if filename.is_empty() {
                        return None;
                    }
                    let size = first_text(value, &["sizeName"]);
                    let size = if size.is_empty() {
                        value.get("size").map(Value::to_string).unwrap_or_default()
                    } else {
                        size
                    };
                    Some(AttachmentMeta {
                        id: id.to_owned(),
                        filename,
                        size,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn attachment(
    item_id: &str,
    attachment_id: &str,
    filename: &str,
    preview: bool,
) -> Result<String, String> {
    let response = kurama_host::attachment::get_attachment(
        item_id,
        attachment_id,
        filename,
        None,
        preview,
        preview,
        None,
        None,
        false,
    );
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "Attachment action failed".to_owned()));
    }
    Ok(response.filename.unwrap_or_else(|| filename.to_owned()))
}

fn item_fields(item: &Value) -> Vec<ItemField> {
    let mut fields = Vec::new();
    match text(item, "type_name").as_str() {
        "login" => {
            if let Some(login) = item.get("login") {
                push_text(
                    &mut fields,
                    "username",
                    "Username",
                    first_text(login, &["username"]),
                );
                push_masked(
                    &mut fields,
                    "password",
                    "Password",
                    !first_text(login, &["password"]).is_empty(),
                );
                push_masked(
                    &mut fields,
                    "totp",
                    "TOTP",
                    !first_text(login, &["totp"]).is_empty(),
                );
                if let Some(uris) = login.get("uris").and_then(Value::as_array) {
                    for (index, uri) in uris.iter().enumerate() {
                        push_text(
                            &mut fields,
                            &format!("uri-{index}"),
                            "Website",
                            first_text(uri, &["uri"]),
                        );
                    }
                }
            }
        }
        "card" => {
            if let Some(card) = item.get("card") {
                push_text(
                    &mut fields,
                    "cardholder",
                    "Cardholder",
                    first_text(card, &["cardholderName", "cardholder_name"]),
                );
                push_text(&mut fields, "brand", "Brand", first_text(card, &["brand"]));
                push_masked(
                    &mut fields,
                    "number",
                    "Number",
                    !first_text(card, &["number"]).is_empty(),
                );
                let month = first_text(card, &["expMonth", "exp_month"]);
                let year = first_text(card, &["expYear", "exp_year"]);
                let expiry = match (month.is_empty(), year.is_empty()) {
                    (false, false) => format!("{month}/{year}"),
                    (false, true) => month,
                    (true, false) => year,
                    (true, true) => String::new(),
                };
                push_text(&mut fields, "expiry", "Expires", expiry);
                push_masked(
                    &mut fields,
                    "code",
                    "Security code",
                    !first_text(card, &["code"]).is_empty(),
                );
            }
        }
        "identity" => {
            if let Some(identity) = item.get("identity") {
                let first = first_text(identity, &["firstName", "first_name"]);
                let last = first_text(identity, &["lastName", "last_name"]);
                let full = match (first.is_empty(), last.is_empty()) {
                    (false, false) => format!("{first} {last}"),
                    (false, true) => first,
                    (true, false) => last,
                    (true, true) => String::new(),
                };
                push_text(&mut fields, "name", "Name", full);
                push_text(
                    &mut fields,
                    "email",
                    "Email",
                    first_text(identity, &["email"]),
                );
                push_text(
                    &mut fields,
                    "phone",
                    "Phone",
                    first_text(identity, &["phone"]),
                );
            }
        }
        "ssh_key" => {
            if let Some(key) = item.get("ssh_key") {
                push_text(
                    &mut fields,
                    "key-type",
                    "Key type",
                    first_text(key, &["key_type", "keyType"]),
                );
                push_text(
                    &mut fields,
                    "fingerprint",
                    "Fingerprint",
                    first_text(key, &["fingerprint", "keyFingerprint"]),
                );
                push_text(
                    &mut fields,
                    "public-key",
                    "Public key",
                    first_text(key, &["public_key", "publicKey"]),
                );
                push_masked(
                    &mut fields,
                    "private-key",
                    "Private key",
                    !first_text(key, &["private_key", "privateKey"]).is_empty(),
                );
                push_masked(
                    &mut fields,
                    "passphrase",
                    "Passphrase",
                    !first_text(key, &["passphrase"]).is_empty(),
                );
            }
        }
        _ => {}
    }
    push_masked(
        &mut fields,
        "notes",
        "Notes",
        !text(item, "notes").is_empty(),
    );
    if let Some(values) = item.get("fields").and_then(Value::as_array) {
        for (index, field) in values.iter().enumerate() {
            let name = first_text(field, &["name"]);
            if !name.is_empty() {
                push_masked(&mut fields, &format!("custom-{index}"), &name, true);
            }
        }
    }
    push_text(&mut fields, "created", "Created", text(item, "created_at"));
    push_text(&mut fields, "updated", "Modified", text(item, "updated_at"));
    fields
}

fn first_text(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            let found = text(value, key);
            (!found.is_empty()).then_some(found)
        })
        .unwrap_or_default()
}

fn totp_response(item_id: &str) -> Result<(TotpMeta, String), String> {
    let response = call(json!({ "action": "totp", "query": item_id }))?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(text(&response, "error"));
    }
    let meta = TotpMeta {
        item_id: item_id.to_owned(),
        ttl: response.get("ttl").and_then(Value::as_u64).unwrap_or(0),
        period: response.get("period").and_then(Value::as_u64).unwrap_or(30),
    };
    Ok((meta, text(&response, "code")))
}

fn revealed_field(item_id: &str, key: &str) -> Result<String, String> {
    let response = call(json!({ "action": "get_item", "query": item_id }))?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(text(&response, "error"));
    }
    let item = response
        .get("item")
        .ok_or_else(|| "Item details are unavailable".to_owned())?;
    value_for(item, key).ok_or_else(|| "Field is no longer available".to_owned())
}

fn value_for(item: &Value, key: &str) -> Option<String> {
    let found = if let Some(index) = key.strip_prefix("uri-") {
        let uris = item.get("login")?.get("uris")?.as_array()?;
        first_text(uris.get(index.parse::<usize>().ok()?)?, &["uri"])
    } else if let Some(index) = key.strip_prefix("custom-") {
        let values = item.get("fields")?.as_array()?;
        first_text(values.get(index.parse::<usize>().ok()?)?, &["value"])
    } else {
        let section = match key {
            "username" | "password" | "totp" => item.get("login")?,
            "cardholder" | "brand" | "number" | "expiry" | "code" => item.get("card")?,
            "name" | "email" | "phone" => item.get("identity")?,
            "key-type" | "fingerprint" | "public-key" | "private-key" | "passphrase" => {
                item.get("ssh_key")?
            }
            "notes" => {
                let notes = text(item, "notes");
                return (!notes.is_empty()).then_some(notes);
            }
            _ => return None,
        };
        match key {
            "username" | "password" | "totp" | "brand" | "number" | "code" | "email" | "phone"
            | "passphrase" => first_text(section, &[key]),
            "cardholder" => first_text(section, &["cardholderName", "cardholder_name"]),
            "expiry" => join_parts(
                &[
                    first_text(section, &["expMonth", "exp_month"]),
                    first_text(section, &["expYear", "exp_year"]),
                ],
                "/",
            ),
            "name" => join_parts(
                &[
                    first_text(section, &["firstName", "first_name"]),
                    first_text(section, &["lastName", "last_name"]),
                ],
                " ",
            ),
            "key-type" => first_text(section, &["key_type", "keyType"]),
            "fingerprint" => first_text(section, &["fingerprint", "keyFingerprint"]),
            "public-key" => first_text(section, &["public_key", "publicKey"]),
            "private-key" => first_text(section, &["private_key", "privateKey"]),
            _ => return None,
        }
    };
    (!found.is_empty()).then_some(found)
}

fn join_parts(parts: &[String], separator: &str) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(separator)
}

fn push_text(fields: &mut Vec<ItemField>, key: &str, label: &str, value: String) {
    if !value.is_empty() {
        fields.push(ItemField {
            key: key.to_owned(),
            label: label.to_owned(),
            value: FieldValue::Text(value),
        });
    }
}

fn push_masked(fields: &mut Vec<ItemField>, key: &str, label: &str, present: bool) {
    if present {
        fields.push(ItemField {
            key: key.to_owned(),
            label: label.to_owned(),
            value: FieldValue::Masked,
        });
    }
}

fn notices(response: &Value) -> (u64, Vec<Notice>) {
    let cursor = response.get("cursor").and_then(Value::as_u64).unwrap_or(0);
    let values = response.get("events").and_then(Value::as_array);
    let notices = values
        .map(|values| values.iter().filter_map(notice).collect())
        .unwrap_or_default();
    (cursor, notices)
}

fn notice(event: &Value) -> Option<Notice> {
    let kind = event.get("kind")?.as_str()?;
    let (message, approval_id) = match kind {
        "status" => match event.get("status").and_then(Value::as_str) {
            Some("unlocked") => ("Vault unlocked", None),
            Some("locked") => ("Vault locked", None),
            _ => return None,
        },
        "sync" => match event.get("phase").and_then(Value::as_str) {
            Some("started") => ("Syncing vault…", None),
            Some("completed") => ("Vault synced", None),
            _ => return None,
        },
        "error" => {
            let detail = event.get("message").and_then(Value::as_str).unwrap_or("");
            let message = match detail {
                "Vault unlock failed" => "Vault unlock failed",
                "Quick unlock failed" => "Quick unlock failed",
                "Vault sync failed" => "Vault sync failed",
                _ => "Backend request failed",
            };
            (message, None)
        }
        "approval" => (
            "SSH approval requested",
            event.get("prompt_id").and_then(Value::as_u64),
        ),
        _ => return None,
    };
    Some(Notice {
        kind: kind.to_owned(),
        message: message.to_owned(),
        approval_id,
    })
}

fn approvals(response: &Value) -> Vec<Approval> {
    response
        .get("prompts")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(approval).collect())
        .unwrap_or_default()
}

fn approval(value: &Value) -> Option<Approval> {
    Some(Approval {
        id: value.get("id")?.as_u64()?,
        title: text(value, "title"),
        action: text(value, "action"),
        key_name: text(value, "key_name"),
        client: text(value, "client"),
    })
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_is_redacted() {
        let secret = Secret::new("hunter2".to_owned());
        let debug = format!("{:?}", secret);
        assert!(!debug.contains("hunter2"));
        assert!(debug.contains("redacted"));
    }

    #[test]
    fn rows_keep_metadata_but_drop_secret_fields() {
        let response = json!([{
            "id": "item-1",
            "name": "Example",
            "type_name": "login",
            "sub_title": "user@example.com",
            "favorite": true,
            "folder_name": "Work",
            "updated_at": "2026-01-02T03:04:05Z",
            "login": { "username": "user@example.com", "password": "super-secret" },
            "notes": "private note"
        }]);
        let items = rows(&response).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Example");
        assert_eq!(items[0].subtitle, "user@example.com");
        assert!(items[0].favorite);
        let debug = format!("{:?}", items);
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("private note"));
    }

    #[test]
    fn attachments_keep_metadata_but_drop_keys() {
        let item = json!({
            "attachments": [{
                "id": "att-1",
                "fileName": "receipt.pdf",
                "sizeName": "2.4 KB",
                "key": "attachment-secret-key",
                "url": "https://vault.example.com/attachments/att-1"
            }]
        });
        let attachments = attachment_metadata(&item);
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].id, "att-1");
        assert_eq!(attachments[0].filename, "receipt.pdf");
        assert_eq!(attachments[0].size, "2.4 KB");
        let debug = format!("{attachments:?}");
        assert!(!debug.contains("attachment-secret-key"));
        assert!(!debug.contains("vault.example.com"));
    }

    #[test]
    fn trash_rows_keep_deleted_item_metadata() {
        let response = json!({
            "ok": true,
            "items": [{
                "id": "trash-1",
                "name": "Old item",
                "type_name": "note",
                "folder_name": "Personal",
                "login": { "password": "super-secret" }
            }]
        });
        let items = trash_rows(&response).unwrap();
        assert_eq!(items[0].id, "trash-1");
        assert_eq!(items[0].folder, "Personal");
        let debug = format!("{:?}", items);
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn events_keep_known_messages_only() {
        let response = json!({
            "ok": true,
            "cursor": 6,
            "events": [
                { "id": 4, "kind": "status", "status": "unlocked" },
                { "id": 5, "kind": "error", "message": "token=super-secret" },
                { "id": 6, "kind": "approval", "prompt_id": 9, "view": "ssh-approval" },
                { "id": 7, "kind": "unknown", "message": "ignored" }
            ]
        });
        let (cursor, found) = notices(&response);
        assert_eq!(cursor, 6);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].message, "Vault unlocked");
        assert_eq!(found[1].message, "Backend request failed");
        assert_eq!(found[2].approval_id, Some(9));
        let debug = format!("{:?}", found);
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn approvals_drop_command_and_fingerprint() {
        let response = json!({
            "ok": true,
            "prompts": [{
                "id": 7,
                "title": "Sign a Git commit?",
                "action": "SSH login as git@github.com",
                "key_name": "Laptop key",
                "client": "git (ssh)",
                "command": "ssh git@github.com super-secret",
                "fingerprint": "SHA256:secretfp"
            }]
        });
        let found = approvals(&response);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Sign a Git commit?");
        let debug = format!("{:?}", found);
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("secretfp"));
    }

    #[test]
    fn item_fields_mask_secrets_but_keep_field_labels() {
        let item = json!({
            "id": "item-1",
            "type_name": "login",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T03:04:05Z",
            "notes": "private note",
            "login": {
                "username": "dev@example.com",
                "password": "super-secret",
                "totp": "otpauth://totp/example?secret=private-seed",
                "uris": [{ "uri": "https://github.com" }]
            },
            "fields": [{ "name": "Recovery code", "value": "private-code" }]
        });
        let fields = item_fields(&item);
        assert!(fields.contains(&ItemField {
            key: "username".to_owned(),
            label: "Username".to_owned(),
            value: FieldValue::Text("dev@example.com".to_owned()),
        }));
        assert!(fields.contains(&ItemField {
            key: "uri-0".to_owned(),
            label: "Website".to_owned(),
            value: FieldValue::Text("https://github.com".to_owned()),
        }));
        assert!(fields.contains(&ItemField {
            key: "created".to_owned(),
            label: "Created".to_owned(),
            value: FieldValue::Text("2026-01-01T00:00:00Z".to_owned()),
        }));
        assert!(fields.contains(&ItemField {
            key: "updated".to_owned(),
            label: "Modified".to_owned(),
            value: FieldValue::Text("2026-01-02T03:04:05Z".to_owned()),
        }));
        for key in ["password", "totp", "notes", "custom-0"] {
            let found = fields.iter().find(|field| field.key == key).unwrap();
            assert_eq!(found.value, FieldValue::Masked);
        }
        let debug = format!("{:?}", fields);
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("private note"));
        assert!(!debug.contains("private-seed"));
        assert!(!debug.contains("private-code"));
    }

    #[test]
    fn ssh_item_fields_keep_public_metadata_only() {
        let item = json!({
            "type_name": "ssh_key",
            "ssh_key": {
                "key_type": "ed25519",
                "fingerprint": "SHA256:publicfp",
                "public_key": "ssh-ed25519 AAAA public",
                "private_key": "secret-private-material",
                "passphrase": "secret-passphrase-value"
            }
        });
        let fields = item_fields(&item);
        assert_eq!(fields[0].value, FieldValue::Text("ed25519".to_owned()));
        assert_eq!(
            fields[1].value,
            FieldValue::Text("SHA256:publicfp".to_owned())
        );
        assert_eq!(fields[2].label, "Public key");
        assert_eq!(fields[3].value, FieldValue::Masked);
        assert_eq!(fields[4].value, FieldValue::Masked);
        let debug = format!("{:?}", fields);
        assert!(!debug.contains("secret-private-material"));
        assert!(!debug.contains("secret-passphrase-value"));
    }

    #[test]
    fn totp_store_is_keyed_by_item() {
        stash_totp("backend-totp-test", "123456".to_owned());
        assert_eq!(totp_value("backend-totp-test").as_deref(), Some("123456"));
        assert_eq!(totp_value("other"), None);
        clear_totp("backend-totp-test");
        assert_eq!(totp_value("backend-totp-test"), None);
    }

    #[test]
    fn value_for_reads_each_supported_field() {
        let item = json!({
            "notes": "private note",
            "login": {
                "password": "super-secret",
                "uris": [{ "uri": "https://github.com" }]
            },
            "fields": [{ "name": "Recovery code", "value": "private-code" }]
        });
        assert_eq!(
            value_for(&item, "password").as_deref(),
            Some("super-secret")
        );
        assert_eq!(
            value_for(&item, "uri-0").as_deref(),
            Some("https://github.com")
        );
        assert_eq!(value_for(&item, "notes").as_deref(), Some("private note"));
        assert_eq!(
            value_for(&item, "custom-0").as_deref(),
            Some("private-code")
        );
        assert_eq!(value_for(&item, "unknown"), None);
    }

    #[test]
    fn revealed_values_are_keyed_and_clearable() {
        let _guard = REVEAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_revealed();
        stash_revealed("store-test-key", "super-secret".to_owned());
        assert_eq!(
            revealed_value("store-test-key").as_deref(),
            Some("super-secret")
        );
        assert_eq!(revealed_value("notes"), None);
        clear_revealed();
        assert_eq!(revealed_value("store-test-key"), None);
    }
}
