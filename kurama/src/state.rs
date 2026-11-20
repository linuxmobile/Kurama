use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, Sender};

use rune::{edit::TextEdit, Tx, Waker};

use crate::backend::{self, Approval, Event, Request, Secret, Snapshot, VaultRow};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
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
    pub password: TextEdit,
    pub category: Category,
    pub selected: Option<String>,
    pub busy: bool,
    pub notice: Option<String>,
    pub approvals: Vec<Approval>,
    pub dark_override: Option<bool>,
    rows_version: u64,
    visible_cache: RefCell<Option<VisibleCache>>,
    requests: Option<Sender<Request>>,
    events: Option<Receiver<Event>>,
    event_cursor: u64,
    event_timer: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            rows: Vec::new(),
            query: TextEdit::new(""),
            password: TextEdit::new("").secure(),
            category: Category::All,
            selected: None,
            busy: true,
            notice: None,
            approvals: Vec::new(),
            dark_override: None,
            rows_version: 0,
            visible_cache: RefCell::new(None),
            requests: None,
            events: None,
            event_cursor: 0,
            event_timer: 0.0,
        }
    }
}

#[derive(Clone)]
struct VisibleCache {
    version: u64,
    query: String,
    category: Category,
    indices: Vec<usize>,
}

impl State {
    pub fn visible_indices(&self) -> Vec<usize> {
        let query = self.query.text.trim().to_lowercase();
        if let Some(cache) = self.visible_cache.borrow().as_ref() {
            if cache.version == self.rows_version
                && cache.query == query
                && cache.category == self.category
            {
                return cache.indices.clone();
            }
        }

        let indices: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, item)| self.category.matches(item))
            .filter(|(_, item)| {
                query.is_empty()
                    || item.name.to_lowercase().contains(&query)
                    || item.subtitle.to_lowercase().contains(&query)
                    || item.folder.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect();
        *self.visible_cache.borrow_mut() = Some(VisibleCache {
            version: self.rows_version,
            query,
            category: self.category,
            indices: indices.clone(),
        });
        indices
    }

    pub fn rows_version(&self) -> u64 {
        self.rows_version
    }

    fn bump_rows_version(&mut self) {
        self.rows_version = self.rows_version.wrapping_add(1);
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
    state.busy = false;
    state.notice = Some("Backend worker is unavailable.".to_owned());
}

fn send_quiet(state: &State, request: Request) {
    if let Some(sender) = &state.requests {
        let _ = sender.send(request);
    }
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

pub fn poll(state: &mut State, seconds: f64, _tx: &mut Tx) -> bool {
    state.event_timer += seconds;
    if state.event_timer >= 1.0 {
        state.event_timer = 0.0;
        send_quiet(
            state,
            Request::Events {
                since: state.event_cursor,
            },
        );
    }

    let Some(events) = &state.events else {
        return false;
    };
    let queued: Vec<Event> = events.try_iter().collect();
    if queued.is_empty() {
        return false;
    }
    for event in queued {
        apply(state, event);
    }
    true
}

fn apply(state: &mut State, event: Event) {
    state.busy = false;
    match event {
        Event::Snapshot(snapshot) => {
            let changed_vault =
                (state.snapshot.status == "unlocked") != (snapshot.status == "unlocked");
            state.rows = snapshot.items.clone();
            state.bump_rows_version();
            state.snapshot = snapshot;
            state.notice = None;
            if changed_vault {
                state.selected = None;
                state.query.clear();
                state.password.replace_text("");
            }
            if state.selected.is_none() {
                state.selected = state.rows.first().map(|item| item.id.clone());
            }
        }
        Event::Items(items) => {
            state.rows = items;
            state.bump_rows_version();
            if state.selected.is_none() {
                state.selected = state.rows.first().map(|item| item.id.clone());
            }
        }
        Event::Notices { cursor, notices } => {
            state.event_cursor = state.event_cursor.max(cursor);
            let mut refresh = false;
            let mut approvals = false;
            for notice in notices {
                if matches!(notice.kind.as_str(), "status" | "sync") {
                    refresh = true;
                }
                if notice.approval_id.is_some() {
                    approvals = true;
                }
                state.notice = Some(notice.message);
            }
            if approvals {
                send_quiet(state, Request::Approvals);
            }
            if refresh {
                send_quiet(state, Request::Refresh);
            }
        }
        Event::Approvals(items) => {
            state.approvals = items;
            if !state.approvals.is_empty() {
                state.notice = Some("SSH approval requested".to_owned());
            }
        }
        Event::Error(message) => state.notice = Some(message),
    }
}

pub fn submit_search(state: &mut State, _tx: &mut Tx) {
    let query = state.query.text.trim().to_owned();
    dispatch(state, Request::Search(query));
}

pub fn submit_unlock(state: &mut State, _tx: &mut Tx) {
    queue_unlock(state);
}

fn queue_unlock(state: &mut State) {
    let password = state.password.text.clone();
    if password.is_empty() {
        state.notice = Some("Enter your master password.".to_owned());
        return;
    }
    state.password.replace_text("");
    dispatch(
        state,
        Request::Unlock {
            password: Secret::new(password),
        },
    );
}

pub fn submit_quick_unlock(state: &mut State) {
    state.password.replace_text("");
    dispatch(state, Request::QuickUnlock);
}

pub fn scene_populated(state: &mut State) {
    state.snapshot = Snapshot {
        status: "unlocked".to_owned(),
        email: "user@example.com".to_owned(),
        server: "https://vault.example.com".to_owned(),
        last_sync: "2026-01-02T03:04:05Z".to_owned(),
        quick_unlock_ready: false,
        items: Vec::new(),
        items_count: 4,
    };
    state.rows = fixtures();
    state.bump_rows_version();
    state.snapshot.items = state.rows.clone();
    state.selected = state.rows.first().map(|item| item.id.clone());
    state.busy = false;
}

pub fn scene_locked(state: &mut State) {
    state.snapshot = Snapshot {
        status: "locked".to_owned(),
        email: "user@example.com".to_owned(),
        server: "https://vault.example.com".to_owned(),
        quick_unlock_ready: true,
        ..Snapshot::default()
    };
    state.busy = false;
}

pub fn scene_stress(state: &mut State) {
    scene_populated(state);
    state.rows = (0..395)
        .map(|index| VaultRow {
            id: format!("stress-{index}"),
            name: format!("Item {index}"),
            subtitle: "user@example.com".to_owned(),
            type_name: "login".to_owned(),
            favorite: false,
            folder: String::new(),
            updated: "2026-01-02T03:04:05Z".to_owned(),
        })
        .collect();
    state.snapshot.items = state.rows.clone();
    state.snapshot.items_count = state.rows.len();
    state.selected = state.rows.first().map(|item| item.id.clone());
    state.bump_rows_version();
    state.busy = false;
}

pub fn scene_disconnected(state: &mut State) {
    state.busy = false;
    state.notice = Some("Kurama backend is not running.".to_owned());
}

fn fixtures() -> Vec<VaultRow> {
    [
        (
            "1",
            "GitHub",
            "dev@example.com",
            "login",
            true,
            "Development",
        ),
        (
            "2",
            "Cloud console",
            "ops@example.com",
            "login",
            false,
            "Work",
        ),
        ("3", "Recovery codes", "", "note", false, "Personal"),
        ("4", "Laptop SSH key", "ed25519", "ssh_key", false, ""),
    ]
    .into_iter()
    .map(
        |(id, name, subtitle, type_name, favorite, folder)| VaultRow {
            id: id.to_owned(),
            name: name.to_owned(),
            subtitle: subtitle.to_owned(),
            type_name: type_name.to_owned(),
            favorite,
            folder: folder.to_owned(),
            updated: "2026-01-02T03:04:05Z".to_owned(),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_category_and_search_text() {
        let mut state = State::default();
        scene_populated(&mut state);
        assert_eq!(state.visible_indices().len(), 4);
        state.category = Category::Logins;
        assert_eq!(state.visible_indices().len(), 2);
        state.category = Category::All;
        state.query = TextEdit::new("github");
        assert_eq!(state.visible_indices().len(), 1);
    }

    #[test]
    fn selected_item_uses_backend_id() {
        let mut state = State::default();
        scene_populated(&mut state);
        assert_eq!(
            state.selected_item().map(|item| item.id.as_str()),
            Some("1")
        );
    }

    #[test]
    fn unlock_submission_dispatches_a_redacted_secret_and_clears_the_field() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        state.password = TextEdit::new("hunter2").secure();

        queue_unlock(&mut state);

        assert!(state.password.text.is_empty());
        let requests: Vec<_> = rx.try_iter().collect();
        assert_eq!(requests.len(), 1);
        match &requests[0] {
            Request::Unlock { password } => {
                assert_eq!(password.as_str(), "hunter2");
                let debug = format!("{:?}", password);
                assert!(!debug.contains("hunter2"));
            }
            other => panic!("unexpected request: {:?}", other),
        }
    }

    #[test]
    fn quick_unlock_clears_the_password_field() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        state.password = TextEdit::new("hunter2").secure();

        submit_quick_unlock(&mut state);

        assert!(state.password.text.is_empty());
        assert!(matches!(rx.try_iter().next(), Some(Request::QuickUnlock)));
    }

    #[test]
    fn locking_clears_the_unlock_field() {
        let mut state = State::default();
        apply(
            &mut state,
            Event::Snapshot(Snapshot {
                status: "unlocked".to_owned(),
                ..Snapshot::default()
            }),
        );
        state.password = TextEdit::new("hunter2").secure();

        apply(
            &mut state,
            Event::Snapshot(Snapshot {
                status: "locked".to_owned(),
                ..Snapshot::default()
            }),
        );

        assert!(state.password.text.is_empty());
    }

    #[test]
    fn backend_notices_advance_cursor_and_request_followups() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        apply(
            &mut state,
            Event::Notices {
                cursor: 5,
                notices: vec![
                    backend::Notice {
                        kind: "status".to_owned(),
                        message: "Vault unlocked".to_owned(),
                        approval_id: None,
                    },
                    backend::Notice {
                        kind: "approval".to_owned(),
                        message: "SSH approval requested".to_owned(),
                        approval_id: Some(9),
                    },
                ],
            },
        );
        assert_eq!(state.event_cursor, 5);
        assert_eq!(state.notice.as_deref(), Some("SSH approval requested"));
        let requests: Vec<Request> = rx.try_iter().collect();
        assert!(requests.iter().any(|r| matches!(r, Request::Refresh)));
        assert!(requests.iter().any(|r| matches!(r, Request::Approvals)));

        apply(
            &mut state,
            Event::Approvals(vec![Approval {
                id: 9,
                title: "Sign a Git commit?".to_owned(),
                action: "SSH login".to_owned(),
                key_name: "Laptop key".to_owned(),
                client: "git (ssh)".to_owned(),
            }]),
        );
        assert_eq!(state.approvals.len(), 1);
    }
}
