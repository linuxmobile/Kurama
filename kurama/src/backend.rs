//! Bridge between Kurama and the `kurama-host` backend.
//!
//! All backend calls happen on a worker thread through the daemon IPC.
//! The UI only ever sees the small, metadata-only structs below; vault secrets
//! stay inside the daemon.

use serde_json::{json, Value};

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

/// Non-secret view of the daemon's `status` response.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub status: String,
    pub email: String,
    pub server: String,
    pub last_sync: String,
    pub items: Vec<VaultRow>,
    pub items_count: usize,
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
    Search(String),
    Sync,
    Lock,
    Events { since: u64 },
    Approvals,
}

#[derive(Debug)]
pub enum Event {
    Snapshot(Snapshot),
    Items(Vec<VaultRow>),
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

    Ok(Snapshot {
        status: state.to_owned(),
        email: text(&status, "user_email"),
        server: text(&status, "server_url"),
        last_sync: text(&status, "last_sync"),
        items,
        items_count,
    })
}

fn rows(response: &Value) -> Result<Vec<VaultRow>, String> {
    let Some(values) = response.as_array() else {
        return Err("Unexpected item list from backend".to_owned());
    };
    Ok(values.iter().filter_map(row).collect())
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
}
