//! Bridge between Kurama and the existing omawarden backend.
//!
//! All backend calls happen on a worker thread through omawarden's daemon IPC.
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

#[derive(Clone, Debug)]
pub enum Request {
    Refresh,
    Search(String),
    Sync,
    Lock,
}

#[derive(Debug)]
pub enum Event {
    Snapshot(Snapshot),
    Items(Vec<VaultRow>),
    Error(String),
}

const DAEMON_HINT: &str = "Omawarden daemon is not running. Start it with `omawarden daemon`.";

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
    }
}

fn call(body: Value) -> Result<Value, String> {
    let response = omawarden::daemon::send_daemon_request(&body).ok_or(DAEMON_HINT)?;
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
    let state = status.get("status").and_then(Value::as_str).unwrap_or("unknown");
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
    Ok(values.iter().filter_map(|value| row(value)).collect())
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
        favorite: value.get("favorite").and_then(Value::as_bool).unwrap_or(false),
        folder: text(value, "folder_name"),
        updated: text(value, "updated_at"),
    })
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
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
}
