//! Bridge between Kurama and the `kurama-host` backend.
//!
//! All backend calls happen on a worker thread through the daemon IPC.
//! State only holds metadata and field keys. A revealed value lives in a
//! dedicated store so it never appears in `State` or its `Debug` output.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use kurama_host::ssh::{generate_keypair, parse_private_key, GeneratedSshKey, SshAlgorithm};
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
    pub folder_id: String,
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

/// Result of an attachment preview or download. Text content lives in `Secret`.
#[derive(Debug)]
pub struct AttachmentResult {
    pub filename: String,
    pub path: String,
    pub text: Option<Secret>,
    pub is_image: bool,
    pub is_text: bool,
}

/// Metadata-only view of one decrypted vault item.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct ItemDetails {
    pub id: String,
    pub fields: Vec<ItemField>,
    pub attachments: Vec<AttachmentMeta>,
}

/// Non-secret folder metadata used by the create and edit forms.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FolderMeta {
    pub id: String,
    pub name: String,
}

/// View of a Send. The access link is kept inside `Secret` so it never
/// appears in `State`'s debug output.
#[derive(Clone, Debug)]
pub struct SendRow {
    pub id: String,
    pub name: String,
    pub disabled: bool,
    pub expiration: String,
    pub max_access: i64,
    pub access_count: i64,
    pub url: Secret,
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

/// Non-secret result of an account authentication request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthOutcome {
    pub ok: bool,
    pub status: Option<String>,
    pub two_factor_required: bool,
    pub two_factor_providers: Vec<i32>,
    pub two_factor_provider: Option<i32>,
    pub new_device_verification_required: bool,
    pub fido2_status: Option<String>,
    pub error: Option<String>,
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

#[derive(Clone)]
pub struct CardInput {
    pub cardholder_name: Option<Secret>,
    pub number: Option<Secret>,
    pub brand: Option<Secret>,
    pub exp_month: Option<Secret>,
    pub exp_year: Option<Secret>,
    pub code: Option<Secret>,
}

impl CardInput {
    fn value(&self) -> Option<Value> {
        let value = json!({
            "cardholder_name": self.cardholder_name.as_ref().map(Secret::as_str),
            "number": self.number.as_ref().map(Secret::as_str),
            "brand": self.brand.as_ref().map(Secret::as_str),
            "exp_month": self.exp_month.as_ref().map(Secret::as_str),
            "exp_year": self.exp_year.as_ref().map(Secret::as_str),
            "code": self.code.as_ref().map(Secret::as_str),
        });
        let used = value
            .as_object()
            .is_some_and(|fields| fields.values().any(|field| !field.is_null()));
        used.then_some(value)
    }
}

impl fmt::Debug for CardInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CardInput([redacted])")
    }
}

#[derive(Clone)]
pub struct CustomFieldInput {
    pub name: String,
    pub value: Secret,
    pub field_type: i64,
}

impl fmt::Debug for CustomFieldInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CustomFieldInput([redacted])")
    }
}

#[derive(Clone)]
pub struct IdentityInput {
    pub first_name: Option<Secret>,
    pub last_name: Option<Secret>,
    pub email: Option<Secret>,
    pub phone: Option<Secret>,
    pub address1: Option<Secret>,
    pub city: Option<Secret>,
    pub postal_code: Option<Secret>,
    pub country: Option<Secret>,
}

impl IdentityInput {
    fn value(&self) -> Option<Value> {
        let value = json!({
            "first_name": self.first_name.as_ref().map(Secret::as_str),
            "last_name": self.last_name.as_ref().map(Secret::as_str),
            "email": self.email.as_ref().map(Secret::as_str),
            "phone": self.phone.as_ref().map(Secret::as_str),
            "address1": self.address1.as_ref().map(Secret::as_str),
            "city": self.city.as_ref().map(Secret::as_str),
            "postal_code": self.postal_code.as_ref().map(Secret::as_str),
            "country": self.country.as_ref().map(Secret::as_str),
        });
        let used = value
            .as_object()
            .is_some_and(|fields| fields.values().any(|field| !field.is_null()));
        used.then_some(value)
    }
}

impl fmt::Debug for IdentityInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityInput([redacted])")
    }
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
    UpdateItem {
        id: String,
        name: String,
        favorite: bool,
        folder_id: Option<String>,
        username: Option<String>,
        password: Option<Secret>,
        uris: Vec<String>,
        notes: Option<Secret>,
        card: Option<CardInput>,
        identity: Option<IdentityInput>,
    },
    CreateItem {
        name: String,
        item_type: i64,
        favorite: bool,
        folder_id: Option<String>,
        username: Option<String>,
        password: Option<Secret>,
        uris: Vec<String>,
        notes: Option<Secret>,
        card: Option<CardInput>,
        identity: Option<IdentityInput>,
        fields: Vec<CustomFieldInput>,
    },
    CreateSshKey {
        name: String,
        private_key: Secret,
        public_key: Option<String>,
        fingerprint: Option<String>,
        notes: Option<Secret>,
        folder_id: Option<String>,
        favorite: bool,
    },
    Folders,
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
    Logout,
    LoginPassword {
        server_url: String,
        email: String,
        password: Secret,
        code: Option<Secret>,
        two_factor_provider: Option<i32>,
        new_device_otp: Option<Secret>,
    },
    LoginApiKey {
        server_url: String,
        client_id: Secret,
        client_secret: Secret,
    },
    SendTwoFactorEmail {
        server_url: String,
        email: String,
        password: Secret,
    },
    GeneratePassword {
        passphrase: bool,
        length: usize,
        words: usize,
        separator: String,
        capitalize: bool,
        uppercase: bool,
        lowercase: bool,
        numbers: bool,
        special: bool,
        avoid_ambiguous: bool,
    },
    CopyText {
        text: Secret,
        label: String,
    },
    Unlock {
        password: Secret,
    },
    QuickUnlock,
    Events {
        since: u64,
    },
    Approvals,
    ListSends,
    CreateSend {
        name: String,
        text: Secret,
        hidden: bool,
        max_access: Option<u32>,
        expiration: Option<String>,
        password: Option<Secret>,
    },
    DisableSend {
        id: String,
    },
    DeleteSend {
        id: String,
    },
    GenerateSshKey {
        algorithm: String,
        comment: String,
    },
    ImportSshKey {
        private_key: Secret,
    },
    ExportSshKey {
        item_id: String,
    },
    AgentStatus,
    AgentApply,
    AgentReply {
        id: u64,
        approve: bool,
    },
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

static SEND_URLS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn stash_send_url(id: &str, url: &Secret) {
    if let Ok(mut urls) = SEND_URLS.lock() {
        urls.get_or_insert_with(HashMap::new)
            .insert(id.to_owned(), url.as_str().to_owned());
    }
}

pub fn send_url(id: &str) -> Option<String> {
    SEND_URLS.lock().ok()?.as_ref()?.get(id).cloned()
}

pub fn clear_send_urls() {
    if let Ok(mut urls) = SEND_URLS.lock() {
        if let Some(urls) = urls.as_mut() {
            urls.clear();
        }
    }
}

static ATTACHMENT_TEXT: Mutex<Option<(String, Secret)>> = Mutex::new(None);

pub fn stash_attachment_text(filename: &str, text: Secret) {
    if let Ok(mut stored) = ATTACHMENT_TEXT.lock() {
        *stored = Some((filename.to_owned(), text));
    }
}

pub fn attachment_text(filename: &str) -> Option<String> {
    let stored = ATTACHMENT_TEXT.lock().ok()?;
    let (stored_filename, text) = stored.as_ref()?;
    (stored_filename == filename).then(|| text.as_str().to_owned())
}

pub fn clear_attachment_text() {
    if let Ok(mut stored) = ATTACHMENT_TEXT.lock() {
        *stored = None;
    }
}

#[derive(Debug)]
pub enum Event {
    Snapshot(Snapshot),
    Items(Vec<VaultRow>),
    Trash(Vec<VaultRow>),
    Details(ItemDetails),
    Revealed {
        item_id: String,
        key: String,
    },
    AttachmentOpened {
        preview: bool,
        result: AttachmentResult,
    },
    Deleted {
        id: String,
        permanent: bool,
    },
    ItemRenamed {
        id: String,
        name: String,
        favorite: bool,
    },
    ItemCreated {
        id: String,
        name: String,
    },
    Folders(Vec<FolderMeta>),
    Restored {
        id: String,
    },
    Notice(String),
    Totp(TotpMeta),
    FavoriteChanged {
        id: String,
        favorite: bool,
    },
    Notices {
        cursor: u64,
        notices: Vec<Notice>,
    },
    Approvals(Vec<Approval>),
    Sends(Vec<SendRow>),
    SendCreated {
        id: String,
        url: Secret,
    },
    SendChanged {
        id: String,
        disabled: bool,
    },
    SendDeleted {
        id: String,
    },
    SshKeyGenerated {
        private: Secret,
        public: String,
        fingerprint: String,
        algorithm: String,
    },
    SshKeyImported {
        public: String,
        fingerprint: String,
    },
    SshKeyExported {
        private: Secret,
        public: String,
        fingerprint: String,
    },
    AgentStatus {
        running: bool,
        socket: String,
    },
    ApprovalAnswered {
        id: u64,
        approve: bool,
    },
    Auth(AuthOutcome),
    Generated {
        value: Secret,
        entropy: f64,
        kind: String,
    },
    Error(String),
    Synced(Snapshot),
    SyncFailed(String),
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
            Ok(result) => Event::AttachmentOpened { preview, result },
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
        Request::UpdateItem {
            id,
            name,
            favorite,
            folder_id,
            username,
            password,
            uris,
            notes,
            card,
            identity,
        } => match call(json!({
            "action": "update_item",
            "id": id.clone(),
            "options": {
                "name": name.clone(),
                "favorite": favorite,
                "folder_id": folder_id,
                "username": username,
                "password": password.as_ref().map(Secret::as_str),
                "uris": uris,
                "notes": notes.as_ref().map(Secret::as_str),
                "card": card.as_ref().and_then(CardInput::value),
                "identity": identity.as_ref().and_then(IdentityInput::value)
            }
        })) {
            Ok(response) if response.get("ok").and_then(Value::as_bool) == Some(true) => {
                Event::ItemRenamed { id, name, favorite }
            }
            Ok(response) => Event::Error(text(&response, "error")),
            Err(error) => Event::Error(error),
        },
        Request::CreateItem {
            name,
            item_type,
            favorite,
            folder_id,
            username,
            password,
            uris,
            notes,
            card,
            identity,
            fields,
        } => match call(json!({
            "action": "create_item",
            "options": {
                "name": name.clone(),
                "item_type": item_type,
                "favorite": favorite,
                "folder_id": folder_id,
                "username": username,
                "password": password.as_ref().map(Secret::as_str),
                "uris": uris,
                "notes": notes.as_ref().map(Secret::as_str),
                "card": card.as_ref().and_then(CardInput::value),
                "identity": identity.as_ref().and_then(IdentityInput::value),
                "fields": fields
                    .iter()
                    .map(|field| json!({
                        "name": field.name,
                        "value": field.value.as_str(),
                        "type": field.field_type
                    }))
                    .collect::<Vec<_>>()
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
        Request::CreateSshKey {
            name,
            private_key,
            public_key,
            fingerprint,
            notes,
            folder_id,
            favorite,
        } => match call(json!({
            "action": "ssh_key_create",
            "name": name.clone(),
            "private_key": private_key.as_str(),
            "public_key": public_key,
            "fingerprint": fingerprint,
            "notes": notes.as_ref().map(Secret::as_str),
            "folder_id": folder_id
        })) {
            Ok(response) if response.get("ok").and_then(Value::as_bool) == Some(true) => {
                let id = response
                    .get("item")
                    .and_then(|item| item.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                if favorite && !id.is_empty() {
                    let _ = call(json!({
                        "action": "update_item",
                        "id": id.clone(),
                        "options": { "favorite": true }
                    }));
                }
                Event::ItemCreated { id, name }
            }
            Ok(response) => Event::Error(text(&response, "error")),
            Err(error) => Event::Error(error),
        },
        Request::Folders => match folders() {
            Ok(folders) => Event::Folders(folders),
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
                Ok(snapshot) => Event::Synced(snapshot),
                Err(error) => Event::SyncFailed(error),
            },
            Err(error) => Event::SyncFailed(error),
        },
        Request::Lock => match call(json!({ "action": "lock" })) {
            Ok(_) => match snapshot() {
                Ok(snapshot) => Event::Snapshot(snapshot),
                Err(error) => Event::Error(error),
            },
            Err(error) => Event::Error(error),
        },
        Request::Logout => match call(json!({ "action": "logout" })) {
            Ok(response) => Event::Auth(auth_outcome(&response)),
            Err(error) => Event::Auth(AuthOutcome {
                error: Some(error),
                ..AuthOutcome::default()
            }),
        },
        Request::LoginPassword {
            server_url,
            email,
            password,
            code,
            two_factor_provider,
            new_device_otp,
        } => {
            let body = json!({
                "action": "login_password",
                "server_url": server_url,
                "email": email,
                "password": password.as_str(),
                "code": code.as_ref().map(Secret::as_str),
                "two_factor_provider": two_factor_provider,
                "new_device_otp": new_device_otp.as_ref().map(Secret::as_str)
            });
            match call_raw(body) {
                Ok(response) => Event::Auth(auth_outcome(&response)),
                Err(error) => Event::Auth(AuthOutcome {
                    error: Some(error),
                    ..AuthOutcome::default()
                }),
            }
        }
        Request::LoginApiKey {
            server_url,
            client_id,
            client_secret,
        } => {
            let body = json!({
                "action": "login_apikey",
                "server_url": server_url,
                "client_id": client_id.as_str(),
                "client_secret": client_secret.as_str()
            });
            match call_raw(body) {
                Ok(response) => Event::Auth(auth_outcome(&response)),
                Err(error) => Event::Auth(AuthOutcome {
                    error: Some(error),
                    ..AuthOutcome::default()
                }),
            }
        }
        Request::SendTwoFactorEmail {
            server_url,
            email,
            password,
        } => {
            let body = json!({
                "action": "send_two_factor_email",
                "server_url": server_url,
                "email": email,
                "password": password.as_str()
            });
            match call_raw(body) {
                Ok(response) if response.get("ok").and_then(Value::as_bool) != Some(false) => {
                    Event::Notice("Two-factor code sent to your email.".to_owned())
                }
                Ok(response) => Event::Error(
                    response
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("Backend request failed")
                        .to_owned(),
                ),
                Err(error) => Event::Error(error),
            }
        }
        Request::GeneratePassword {
            passphrase,
            length,
            words,
            separator,
            capitalize,
            uppercase,
            lowercase,
            numbers,
            special,
            avoid_ambiguous,
        } => {
            let body = if passphrase {
                json!({
                    "action": "generate_password",
                    "passphrase": true,
                    "words": words,
                    "separator": separator,
                    "capitalize": capitalize
                })
            } else {
                json!({
                    "action": "generate_password",
                    "length": length,
                    "uppercase": uppercase,
                    "lowercase": lowercase,
                    "numbers": numbers,
                    "special": special,
                    "avoid_ambiguous": avoid_ambiguous
                })
            };
            match call(body) {
                Ok(response) => {
                    let value = response
                        .get("password")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    if value.is_empty() {
                        Event::Error("Password generator returned no value".to_owned())
                    } else {
                        Event::Generated {
                            value: Secret::new(value),
                            entropy: response
                                .get("entropy")
                                .and_then(Value::as_f64)
                                .unwrap_or(0.0),
                            kind: response
                                .get("type")
                                .and_then(Value::as_str)
                                .unwrap_or("password")
                                .to_owned(),
                        }
                    }
                }
                Err(error) => Event::Error(error),
            }
        }
        Request::CopyText { text, label } => {
            match call(json!({
                "action": "copy",
                "text": text.as_str(),
                "sensitive": true
            })) {
                Ok(_) => Event::Notice(format!("{label} copied to clipboard")),
                Err(error) => Event::Error(error),
            }
        }
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
        Request::ListSends => match call(json!({ "action": "send_list" })) {
            Ok(response) => Event::Sends(send_rows(&response)),
            Err(error) => Event::Error(error),
        },
        Request::CreateSend {
            name,
            text: share_text,
            hidden,
            max_access,
            expiration,
            password,
        } => match call(json!({
            "action": "send_create",
            "name": name,
            "text": share_text.as_str(),
            "hidden": hidden,
            "max_access": max_access,
            "expiration": expiration,
            "password": password.as_ref().map(Secret::as_str),
        })) {
            Ok(response) => {
                let send = response.get("send").unwrap_or(&response);
                Event::SendCreated {
                    id: text(send, "id"),
                    url: Secret::new(first_text(send, &["accessUrl", "accessURL"])),
                }
            }
            Err(error) => Event::Error(error),
        },
        Request::DisableSend { id } => match call(json!({
            "action": "send_deactivate",
            "id": id.clone(),
        })) {
            Ok(_) => Event::SendChanged { id, disabled: true },
            Err(error) => Event::Error(error),
        },
        Request::DeleteSend { id } => match call(json!({
            "action": "send_delete",
            "id": id.clone(),
        })) {
            Ok(_) => Event::SendDeleted { id },
            Err(error) => Event::Error(error),
        },
        Request::GenerateSshKey { algorithm, comment } => {
            let parsed = match algorithm.parse::<SshAlgorithm>() {
                Ok(parsed) => parsed,
                Err(error) => return Event::Error(error),
            };
            let comment = (!comment.trim().is_empty()).then(|| comment.trim());
            match generate_keypair(parsed, comment) {
                Ok(key) => Event::SshKeyGenerated {
                    private: Secret::new(key.private_key),
                    public: key.public_key,
                    fingerprint: key.fingerprint,
                    algorithm: key.algorithm_name,
                },
                Err(error) => Event::Error(error),
            }
        }
        Request::ImportSshKey { private_key } => match parse_private_key(private_key.as_str()) {
            Ok(key) => Event::SshKeyImported {
                public: key.public_key,
                fingerprint: key.fingerprint,
            },
            Err(error) => Event::Error(error),
        },
        Request::ExportSshKey { item_id } => match call(json!({
            "action": "get_ssh_key",
            "query": item_id.clone(),
        })) {
            Ok(response) => match exported_ssh_key(&response) {
                Ok(key) => Event::SshKeyExported {
                    private: Secret::new(key.private_key),
                    public: key.public_key,
                    fingerprint: key.fingerprint,
                },
                Err(error) => Event::Error(error),
            },
            Err(error) => Event::Error(error),
        },
        Request::AgentStatus => match call(json!({ "action": "ssh_agent_status" })) {
            Ok(response) => Event::AgentStatus {
                running: response
                    .get("running")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                socket: text(&response, "socket"),
            },
            Err(error) => Event::Error(error),
        },
        Request::AgentApply => match call(json!({ "action": "ssh_agent_apply" })) {
            Ok(response) => Event::AgentStatus {
                running: response
                    .get("running")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                socket: String::new(),
            },
            Err(error) => Event::Error(error),
        },
        Request::AgentReply { id, approve } => match call(json!({
            "action": "ssh_agent_reply",
            "id": id,
            "approve": approve,
        })) {
            Ok(_) => Event::ApprovalAnswered { id, approve },
            Err(error) => Event::Error(error),
        },
    }
}

fn auth_outcome(response: &Value) -> AuthOutcome {
    AuthOutcome {
        ok: response.get("ok").and_then(Value::as_bool).unwrap_or(false),
        status: response
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_owned),
        two_factor_required: response
            .get("two_factor_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        two_factor_providers: response
            .get("two_factor_providers")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_i64)
                    .filter_map(|v| i32::try_from(v).ok())
                    .collect()
            })
            .unwrap_or_default(),
        two_factor_provider: response
            .get("two_factor_provider")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        new_device_verification_required: response
            .get("new_device_verification_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        fido2_status: response
            .get("fido2_status")
            .and_then(Value::as_str)
            .map(str::to_owned),
        error: response
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

fn call_raw(body: Value) -> Result<Value, String> {
    kurama_host::daemon::send_daemon_request(&body).ok_or_else(|| DAEMON_HINT.to_owned())
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
        folder_id: text(value, "folder_id"),
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

fn folders() -> Result<Vec<FolderMeta>, String> {
    let response = call(json!({ "action": "folders" }))?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(text(&response, "error"));
    }
    Ok(response
        .get("folders")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    let id = value.get("id")?.as_str()?;
                    let name = value.get("name")?.as_str()?;
                    Some(FolderMeta {
                        id: id.to_owned(),
                        name: name.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
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
) -> Result<AttachmentResult, String> {
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
    Ok(AttachmentResult {
        filename: response.filename.unwrap_or_else(|| filename.to_owned()),
        path: response.path.unwrap_or_default(),
        text: response
            .text_content
            .filter(|text| !text.is_empty())
            .map(Secret::new),
        is_image: response.is_image.unwrap_or(false),
        is_text: response.is_text.unwrap_or(false),
    })
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

fn send_rows(response: &Value) -> Vec<SendRow> {
    response
        .get("sends")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(send_row).collect())
        .unwrap_or_default()
}

fn send_row(value: &Value) -> Option<SendRow> {
    let name = text(value, "name");
    Some(SendRow {
        id: text(value, "id"),
        name: if name.is_empty() {
            "(unnamed send)".to_owned()
        } else {
            name
        },
        disabled: value
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        expiration: first_text(value, &["expirationDate", "expiration"]),
        max_access: value
            .get("maxAccessCount")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        access_count: value
            .get("accessCount")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        url: Secret::new(first_text(value, &["accessUrl", "accessURL"])),
    })
}

fn exported_ssh_key(response: &Value) -> Result<GeneratedSshKey, String> {
    let item = response
        .get("item")
        .ok_or_else(|| "SSH key item is missing".to_owned())?;
    let ssh = item
        .get("ssh_key")
        .ok_or_else(|| "Item is not an SSH key".to_owned())?;
    let private_key = text(ssh, "private_key");
    if private_key.is_empty() {
        return Err("SSH key item has no private key".to_owned());
    }
    Ok(GeneratedSshKey {
        algorithm_name: text(ssh, "key_type"),
        private_key,
        public_key: text(ssh, "public_key"),
        fingerprint: text(ssh, "fingerprint"),
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
    fn copy_text_request_redacts_value() {
        let request = Request::CopyText {
            text: Secret::new("generated-secret".to_owned()),
            label: "Password".to_owned(),
        };
        let debug = format!("{:?}", request);
        assert!(!debug.contains("generated-secret"));
        assert!(debug.contains("redacted"));
    }

    #[test]
    fn auth_requests_redact_credentials() {
        let request = Request::LoginPassword {
            server_url: "https://vault.example.com".to_owned(),
            email: "user@example.com".to_owned(),
            password: Secret::new("master-secret".to_owned()),
            code: Some(Secret::new("123456".to_owned())),
            two_factor_provider: Some(0),
            new_device_otp: Some(Secret::new("device-secret".to_owned())),
        };
        let debug = format!("{:?}", request);
        assert!(!debug.contains("master-secret"));
        assert!(!debug.contains("123456"));
        assert!(!debug.contains("device-secret"));
        assert!(debug.contains("redacted"));
    }

    #[test]
    fn auth_outcome_keeps_challenge_flags_only() {
        let outcome = auth_outcome(&json!({
            "ok": true,
            "status": "locked",
            "two_factor_required": true,
            "two_factor_providers": [0, 1],
            "two_factor_provider": 0,
            "new_device_verification_required": true,
            "fido2_status": "pending",
            "session": "secret-session",
            "session_token": "secret-token"
        }));
        assert!(outcome.ok);
        assert_eq!(outcome.two_factor_providers, vec![0, 1]);
        assert_eq!(outcome.two_factor_provider, Some(0));
        assert!(outcome.new_device_verification_required);
        assert_eq!(outcome.fido2_status.as_deref(), Some("pending"));
        assert!(!format!("{:?}", outcome).contains("secret-"));
    }

    #[test]
    fn form_requests_redact_passwords() {
        let request = Request::CreateItem {
            name: "Example".to_owned(),
            item_type: 1,
            favorite: false,
            folder_id: None,
            username: None,
            password: Some(Secret::new("hunter2".to_owned())),
            uris: vec!["https://example.com".to_owned()],
            notes: Some(Secret::new("private note".to_owned())),
            card: None,
            identity: Some(IdentityInput {
                first_name: Some(Secret::new("Ada".to_owned())),
                last_name: None,
                email: Some(Secret::new("ada@example.com".to_owned())),
                phone: None,
                address1: None,
                city: None,
                postal_code: None,
                country: None,
            }),
            fields: vec![CustomFieldInput {
                name: "Recovery code".to_owned(),
                value: Secret::new("custom-secret".to_owned()),
                field_type: 1,
            }],
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("hunter2"));
        assert!(!debug.contains("private note"));
        assert!(!debug.contains("ada@example.com"));
        assert!(!debug.contains("custom-secret"));
        assert!(debug.contains("redacted"));
    }

    #[test]
    fn ssh_key_request_redacts_private_key() {
        let request = Request::CreateSshKey {
            name: "Deploy key".to_owned(),
            private_key: Secret::new("PRIVATE KEY MATERIAL".to_owned()),
            public_key: Some("ssh-ed25519 AAAA".to_owned()),
            fingerprint: None,
            notes: Some(Secret::new("ssh note".to_owned())),
            folder_id: None,
            favorite: false,
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("PRIVATE KEY MATERIAL"));
        assert!(!debug.contains("ssh note"));
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
    fn exported_ssh_key_keeps_private_material_redacted() {
        let response = json!({
            "item": {
                "ssh_key": {
                    "key_type": "ed25519",
                    "private_key": "private-key-value",
                    "public_key": "ssh-ed25519 AAAA",
                    "fingerprint": "SHA256:fp"
                }
            }
        });
        let key = exported_ssh_key(&response).unwrap();
        assert_eq!(key.algorithm_name, "ed25519");
        assert_eq!(key.public_key, "ssh-ed25519 AAAA");
        assert_eq!(key.fingerprint, "SHA256:fp");
        let request = Request::ImportSshKey {
            private_key: Secret::new(key.private_key.clone()),
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("private-key-value"));
    }

    #[test]
    fn send_rows_keep_metadata_and_redact_links() {
        let response = json!({
            "sends": [{
                "id": "send-1",
                "name": "Shared note",
                "disabled": false,
                "expirationDate": "2026-02-01T00:00:00Z",
                "maxAccessCount": 5,
                "accessCount": 2,
                "accessUrl": "https://vault.example.com/send#secret"
            }]
        });
        let rows = send_rows(&response);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "send-1");
        assert_eq!(rows[0].name, "Shared note");
        assert_eq!(rows[0].max_access, 5);
        assert_eq!(rows[0].access_count, 2);
        let debug = format!("{rows:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("redacted"));
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
