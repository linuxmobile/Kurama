//! Application state and the update flow between the UI and backend worker.

use std::sync::mpsc::{self, Receiver, Sender};

use rune::{Tx, Waker, edit::TextEdit};

use crate::backend::{self, Event, Request, Snapshot, VaultRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    All,
    Favorites,
    Logins,
    Notes,
    Cards,
    Identities,
    SshKeys,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::All,
        Category::Favorites,
        Category::Logins,
        Category::Notes,
        Category::Cards,
        Category::Identities,
        Category::SshKeys,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::All => "All items",
            Category::Favorites => "Favorites",
            Category::Logins => "Logins",
            Category::Notes => "Secure notes",
            Category::Cards => "Cards",
            Category::Identities => "Identities",
            Category::SshKeys => "SSH keys",
        }
    }

    pub fn matches(self, item: &VaultRow) -> bool {
        match self {
            Category::All => true,
            Category::Favorites => item.favorite,
            Category::Logins => item.type_name == "login",
            Category::Notes => item.type_name == "note",
            Category::Cards => item.type_name == "card",
            Category::Identities => item.type_name == "identity",
            Category::SshKeys => item.type_name == "ssh_key",
        }
    }
}

pub struct State {
    pub snapshot: Snapshot,
    pub rows: Vec<VaultRow>,
    pub query: TextEdit,
    pub category: Category,
    pub selected: Option<String>,
    pub busy: bool,
    pub notice: Option<String>,
    pub dark_override: Option<bool>,
    requests: Option<Sender<Request>>,
    events: Option<Receiver<Event>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            rows: Vec::new(),
            query: TextEdit::new(""),
            category: Category::All,
            selected: None,
            busy: true,
            notice: None,
            dark_override: None,
            requests: None,
            events: None,
        }
    }
}

impl State {
    /// Rows matching the current search and category filters.
    pub fn visible(&self) -> Vec<&VaultRow> {
        let query = self.query.text.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|item| self.category.matches(item))
            .filter(|item| {
                if query.is_empty() {
                    return true;
                }
                item.name.to_lowercase().contains(&query)
                    || item.subtitle.to_lowercase().contains(&query)
                    || item.folder.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn selected_item(&self) -> Option<&VaultRow> {
        let id = self.selected.as_ref()?;
        self.rows.iter().find(|item| &item.id == id)
    }

    pub fn dark(&self, system_dark: bool) -> bool {
        self.dark_override.unwrap_or(system_dark)
    }
}

pub fn dispatch(state: &mut State, request: Request) {
    if let Some(sender) = &state.requests {
        if sender.send(request).is_ok() {
            state.busy = true;
            state.notice = None;
            return;
        }
    }
    // No worker (headless scenes and tests): show the missing bridge instead of
    // silently dropping the action.
    state.busy = false;
    state.notice = Some("Backend worker is unavailable.".to_owned());
}

/// Start the worker once, on the app's setup hook. Every backend call is made
/// here, off the UI thread; results are delivered through `poll`.
pub fn setup(state: &mut State, wake: Waker) {
    let (request_tx, request_rx) = mpsc::channel::<Request>();
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    state.requests = Some(request_tx.clone());
    state.events = Some(event_rx);

    std::thread::spawn(move || {
        while let Ok(request) = request_rx.recv() {
            let event = backend::perform(request);
            if event_tx.send(event).is_ok() {
                wake();
            } else {
                break;
            }
        }
    });

    let _ = request_tx.send(Request::Refresh);
}

pub fn poll(state: &mut State, _seconds: f64, _tx: &mut Tx) -> bool {
    let Some(events) = &state.events else { return false };
    let mut changed = false;
    for event in events.try_iter() {
        changed = true;
        state.busy = false;
        match event {
            Event::Snapshot(snapshot) => {
                let changed_vault =
                    (state.snapshot.status == "unlocked") != (snapshot.status == "unlocked");
                state.rows = snapshot.items.clone();
                state.snapshot = snapshot;
                state.notice = None;
                if changed_vault {
                    state.selected = None;
                    state.query.clear();
                }
                if state.selected.is_none() {
                    state.selected = state.rows.first().map(|item| item.id.clone());
                }
            }
            Event::Items(items) => {
                state.rows = items;
                if state.selected.is_none() {
                    state.selected = state.rows.first().map(|item| item.id.clone());
                }
            }
            Event::Error(message) => state.notice = Some(message),
        }
    }
    changed
}

pub fn submit_search(state: &mut State, _tx: &mut Tx) {
    let query = state.query.text.trim().to_owned();
    dispatch(state, Request::Search(query));
}

pub fn scene_populated(state: &mut State) {
    state.snapshot = Snapshot {
        status: "unlocked".to_owned(),
        email: "user@example.com".to_owned(),
        server: "https://vault.example.com".to_owned(),
        last_sync: "2026-01-02T03:04:05Z".to_owned(),
        items: Vec::new(),
        items_count: 4,
    };
    state.rows = fixtures();
    state.snapshot.items = state.rows.clone();
    state.selected = state.rows.first().map(|item| item.id.clone());
    state.busy = false;
}

pub fn scene_locked(state: &mut State) {
    state.snapshot = Snapshot {
        status: "locked".to_owned(),
        email: "user@example.com".to_owned(),
        server: "https://vault.example.com".to_owned(),
        ..Snapshot::default()
    };
    state.busy = false;
}

pub fn scene_disconnected(state: &mut State) {
    state.busy = false;
    state.notice = Some("Omawarden daemon is not running.".to_owned());
}

fn fixtures() -> Vec<VaultRow> {
    [
        ("1", "GitHub", "dev@example.com", "login", true, "Development"),
        ("2", "Cloud console", "ops@example.com", "login", false, "Work"),
        ("3", "Recovery codes", "", "note", false, "Personal"),
        ("4", "Laptop SSH key", "ed25519", "ssh_key", false, ""),
    ]
    .into_iter()
    .map(|(id, name, subtitle, type_name, favorite, folder)| VaultRow {
        id: id.to_owned(),
        name: name.to_owned(),
        subtitle: subtitle.to_owned(),
        type_name: type_name.to_owned(),
        favorite,
        folder: folder.to_owned(),
        updated: "2026-01-02T03:04:05Z".to_owned(),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_category_and_search_text() {
        let mut state = State::default();
        scene_populated(&mut state);
        assert_eq!(state.visible().len(), 4);
        state.category = Category::Logins;
        assert_eq!(state.visible().len(), 2);
        state.category = Category::All;
        state.query = TextEdit::new("github");
        assert_eq!(state.visible().len(), 1);
    }

    #[test]
    fn selected_item_uses_backend_id() {
        let mut state = State::default();
        scene_populated(&mut state);
        assert_eq!(state.selected_item().map(|item| item.id.as_str()), Some("1"));
    }
}
