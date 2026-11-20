use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, Receiver, Sender};

use rune::{edit::TextEdit, Tx, Waker};

use crate::backend::{
    self, clear_revealed, clear_totp, Approval, Event, FieldValue, ItemDetails, ItemField, Request,
    Secret, Snapshot, TotpMeta, VaultRow,
};

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

    pub fn step(self, delta: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|category| *category == self)
            .unwrap_or(0) as isize;
        let len = Self::ALL.len() as isize;
        Self::ALL[((index + delta).rem_euclid(len)) as usize]
    }

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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Location {
    Vault,
    Trash,
}

pub struct EditDraft {
    pub id: String,
    pub name: TextEdit,
}

pub struct State {
    pub snapshot: Snapshot,
    pub rows: Vec<VaultRow>,
    pub query: TextEdit,
    pub password: TextEdit,
    pub category: Category,
    pub location: Location,
    pub folder: Option<String>,
    pub folders: Vec<String>,
    pub selected: Option<String>,
    pub details: Option<ItemDetails>,
    pub edit: Option<EditDraft>,
    pub revealed: Option<String>,
    pub totp: Option<TotpMeta>,
    pub busy: bool,
    pub notice: Option<String>,
    pub approvals: Vec<Approval>,
    pub pending_delete: Option<String>,
    pub dark_override: Option<bool>,
    rows_version: u64,
    visible_cache: RefCell<Option<VisibleCache>>,
    pub(crate) requests: Option<Sender<Request>>,
    events: Option<Receiver<Event>>,
    event_cursor: u64,
    event_timer: f64,
    totp_timer: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            rows: Vec::new(),
            query: TextEdit::new(""),
            password: TextEdit::new("").secure(),
            category: Category::All,
            location: Location::Vault,
            folder: None,
            folders: Vec::new(),
            selected: None,
            details: None,
            edit: None,
            revealed: None,
            totp: None,
            busy: true,
            notice: None,
            approvals: Vec::new(),
            pending_delete: None,
            dark_override: None,
            rows_version: 0,
            visible_cache: RefCell::new(None),
            requests: None,
            events: None,
            event_cursor: 0,
            event_timer: 0.0,
            totp_timer: 0.0,
        }
    }
}

#[derive(Clone)]
struct VisibleCache {
    version: u64,
    query: String,
    category: Category,
    location: Location,
    folder: Option<String>,
    indices: Vec<usize>,
}

impl State {
    pub fn visible_indices(&self) -> Vec<usize> {
        let query = self.query.text.trim().to_lowercase();
        if let Some(cache) = self.visible_cache.borrow().as_ref() {
            if cache.version == self.rows_version
                && cache.query == query
                && cache.category == self.category
                && cache.location == self.location
                && cache.folder == self.folder
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
                self.folder
                    .as_ref()
                    .is_none_or(|folder| folder == &item.folder)
            })
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
            location: self.location,
            folder: self.folder.clone(),
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

pub fn select_category(state: &mut State, category: Category) {
    let left_trash = leave_trash(state);
    state.category = category;
    state.folder = None;
    if left_trash {
        dispatch(state, Request::Refresh);
    }
}

pub fn select_folder(state: &mut State, folder: String) {
    let left_trash = leave_trash(state);
    state.category = Category::All;
    state.folder = Some(folder);
    if left_trash {
        dispatch(state, Request::Refresh);
    }
}

pub fn move_selection(state: &mut State, delta: isize, tx: &mut Tx) -> bool {
    let visible = state.visible_indices();
    let Some(first) = visible.first() else {
        return false;
    };
    let current = state.selected.as_ref().and_then(|id| {
        visible
            .iter()
            .position(|&index| state.rows[index].id == *id)
    });
    let index = match current {
        Some(position) => {
            let next = (position as isize + delta).clamp(0, visible.len() as isize - 1);
            visible[next as usize]
        }
        None if delta < 0 => *visible.last().unwrap_or(first),
        None => *first,
    };
    select_row(state, index, tx)
}

pub fn select_first(state: &mut State, tx: &mut Tx) -> bool {
    state
        .visible_indices()
        .first()
        .is_some_and(|&index| select_row(state, index, tx))
}

pub fn select_last(state: &mut State, tx: &mut Tx) -> bool {
    state
        .visible_indices()
        .last()
        .is_some_and(|&index| select_row(state, index, tx))
}

fn select_row(state: &mut State, index: usize, tx: &mut Tx) -> bool {
    let id = state.rows[index].id.clone();
    if state.selected.as_deref() == Some(id.as_str()) {
        return false;
    }
    select_item(state, id.clone());
    tx.scroll_into_view_in("item-scroller", format!("item-{id}"));
    true
}

pub fn select_item(state: &mut State, id: String) {
    if state.selected.as_deref() == Some(id.as_str()) {
        return;
    }
    state.selected = Some(id);
    state.pending_delete = None;
    clear_reveal(state);
    request_details(state);
}

pub fn toggle_reveal(state: &mut State, key: String) {
    if state.revealed.as_deref() == Some(key.as_str()) {
        clear_reveal(state);
        return;
    }
    let Some(item_id) = state.selected.clone() else {
        return;
    };
    clear_reveal(state);
    send_quiet(state, Request::RevealField { item_id, key });
}

pub fn set_favorite(state: &mut State, favorite: bool) {
    let Some(id) = state.selected.clone() else {
        return;
    };
    dispatch(state, Request::SetFavorite { id, favorite });
}

fn clear_reveal(state: &mut State) {
    if state.revealed.take().is_some() {
        clear_revealed();
    }
    if let Some(totp) = state.totp.take() {
        clear_totp(&totp.item_id);
    }
}

pub fn open_attachment(state: &mut State, attachment_id: String, filename: String, preview: bool) {
    let Some(item_id) = state.selected.clone() else {
        return;
    };
    dispatch(
        state,
        Request::Attachment {
            item_id,
            attachment_id,
            filename,
            preview,
        },
    );
}

pub fn start_edit(state: &mut State) {
    let Some(item) = state.selected_item() else {
        return;
    };
    state.edit = Some(EditDraft {
        id: item.id.clone(),
        name: TextEdit::new(&item.name),
    });
}

pub fn submit_edit(state: &mut State) {
    let Some(draft) = state.edit.take() else {
        return;
    };
    let name = draft.name.text.trim().to_owned();
    if name.is_empty() {
        state.notice = Some("Enter a name for this item.".to_owned());
        return;
    }
    dispatch(state, Request::UpdateName { id: draft.id, name });
}

pub fn cancel_edit(state: &mut State) {
    state.edit = None;
}

pub fn delete_item(state: &mut State, permanent: bool) {
    let Some(id) = state.selected.clone() else {
        return;
    };
    if permanent && state.pending_delete.as_deref() != Some(id.as_str()) {
        state.pending_delete = Some(id);
        state.notice = Some("Click again to delete permanently".to_owned());
        return;
    }
    state.pending_delete = None;
    dispatch(state, Request::Delete { id, permanent });
}

pub fn restore_item(state: &mut State) {
    let Some(id) = state.selected.clone() else {
        return;
    };
    dispatch(state, Request::Restore { id });
}

pub fn copy_field(state: &State, key: &str, label: &str, sensitive: bool) {
    let Some(item_id) = state.selected.clone() else {
        return;
    };
    send_quiet(
        state,
        Request::CopyField {
            item_id,
            key: key.to_owned(),
            label: label.to_owned(),
            sensitive,
        },
    );
}

fn request_details(state: &mut State) {
    if state.snapshot.status != "unlocked" {
        return;
    }
    let Some(id) = state.selected.clone() else {
        return;
    };
    if state
        .details
        .as_ref()
        .is_some_and(|details| details.id == id)
    {
        return;
    }
    send_quiet(state, Request::Details { id });
}

pub fn open_trash(state: &mut State) {
    if state.location == Location::Trash {
        return;
    }
    state.location = Location::Trash;
    state.category = Category::All;
    state.folder = None;
    state.query.clear();
    state.rows.clear();
    state.selected = None;
    state.details = None;
    clear_reveal(state);
    state.bump_rows_version();
    dispatch(state, Request::Trash);
}

fn leave_trash(state: &mut State) -> bool {
    if state.location != Location::Trash {
        return false;
    }
    state.location = Location::Vault;
    state.folder = None;
    state.query.clear();
    state.rows.clear();
    state.pending_delete = None;
    state.selected = None;
    state.details = None;
    clear_reveal(state);
    state.bump_rows_version();
    true
}

fn folders_from(items: &[VaultRow]) -> Vec<String> {
    let mut folders = BTreeSet::new();
    for item in items {
        if !item.folder.is_empty() {
            folders.insert(item.folder.clone());
        }
    }
    folders.into_iter().collect()
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
    let before = visible_fingerprint(state);
    tick_totp(state, seconds);
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
        return visible_fingerprint(state) != before;
    };
    let queued: Vec<Event> = events.try_iter().collect();
    if queued.is_empty() {
        return visible_fingerprint(state) != before;
    }
    for event in queued {
        apply(state, event);
    }
    visible_fingerprint(state) != before
}

fn tick_totp(state: &mut State, seconds: f64) {
    let Some(totp) = state.totp.as_mut() else {
        return;
    };
    state.totp_timer += seconds;
    while state.totp_timer >= 1.0 {
        state.totp_timer -= 1.0;
        totp.ttl = totp.ttl.saturating_sub(1);
    }
    if totp.ttl == 0 {
        let item_id = totp.item_id.clone();
        state.totp = None;
        clear_totp(&item_id);
        send_quiet(state, Request::Totp { item_id });
    }
}

fn visible_fingerprint(state: &State) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.snapshot.status.hash(&mut hasher);
    state.snapshot.email.hash(&mut hasher);
    state.snapshot.server.hash(&mut hasher);
    state.snapshot.last_sync.hash(&mut hasher);
    state.snapshot.items_count.hash(&mut hasher);
    state.query.text.hash(&mut hasher);
    state.busy.hash(&mut hasher);
    state.notice.hash(&mut hasher);
    state.dark_override.hash(&mut hasher);
    if state.snapshot.status != "unlocked" {
        state.snapshot.quick_unlock_ready.hash(&mut hasher);
        return hasher.finish();
    }
    state.category.hash(&mut hasher);
    state.location.hash(&mut hasher);
    state.folder.hash(&mut hasher);
    state.selected.hash(&mut hasher);
    state.details.hash(&mut hasher);
    state.revealed.hash(&mut hasher);
    state.totp.hash(&mut hasher);
    for folder in &state.folders {
        folder.hash(&mut hasher);
    }
    for index in state.visible_indices() {
        index.hash(&mut hasher);
        let row = &state.rows[index];
        row.id.hash(&mut hasher);
        row.name.hash(&mut hasher);
        row.subtitle.hash(&mut hasher);
        row.type_name.hash(&mut hasher);
        row.favorite.hash(&mut hasher);
        row.folder.hash(&mut hasher);
        row.updated.hash(&mut hasher);
    }
    for approval in &state.approvals {
        approval.id.hash(&mut hasher);
        approval.title.hash(&mut hasher);
        approval.action.hash(&mut hasher);
        approval.key_name.hash(&mut hasher);
        approval.client.hash(&mut hasher);
    }
    state.pending_delete.hash(&mut hasher);
    state
        .edit
        .as_ref()
        .map(|draft| draft.id.as_str())
        .hash(&mut hasher);
    hasher.finish()
}

fn apply(state: &mut State, event: Event) {
    state.busy = false;
    match event {
        Event::Snapshot(snapshot) => {
            let changed_vault =
                (state.snapshot.status == "unlocked") != (snapshot.status == "unlocked");
            let items = snapshot.items.clone();
            state.snapshot = snapshot;
            state.folders = folders_from(&items);
            if changed_vault {
                state.location = Location::Vault;
                state.folder = None;
                state.selected = None;
                state.details = None;
                state.edit = None;
                clear_reveal(state);
                state.query.clear();
                state.password.replace_text("");
            }
            if state.location != Location::Trash {
                state.rows = items;
                state.bump_rows_version();
            }
            state.notice = None;
            clear_reveal(state);
            if state.selected.is_none() {
                state.selected = state.rows.first().map(|item| item.id.clone());
            }
            request_details(state);
        }
        Event::Items(items) => {
            if state.location == Location::Trash {
                return;
            }
            state.rows = items;
            state.bump_rows_version();
            clear_reveal(state);
            if state.selected.is_none() {
                state.selected = state.rows.first().map(|item| item.id.clone());
            }
            request_details(state);
        }
        Event::Trash(items) => {
            if state.location != Location::Trash {
                return;
            }
            state.rows = items;
            state.bump_rows_version();
            clear_reveal(state);
            if state.selected.is_none() {
                state.selected = state.rows.first().map(|item| item.id.clone());
            }
            request_details(state);
        }
        Event::Details(details) => {
            if state.selected.as_deref() == Some(details.id.as_str()) {
                let has_totp = details.fields.iter().any(|field| field.key == "totp");
                state.details = Some(details);
                if has_totp {
                    send_quiet(
                        state,
                        Request::Totp {
                            item_id: state.selected.clone().unwrap_or_default(),
                        },
                    );
                } else if let Some(previous) = state.totp.take() {
                    state.totp_timer = 0.0;
                    clear_totp(&previous.item_id);
                }
            }
        }
        Event::Revealed { item_id, key } => {
            let known = state.selected.as_deref() == Some(item_id.as_str())
                && state
                    .details
                    .as_ref()
                    .is_some_and(|details| details.fields.iter().any(|field| field.key == key));
            if known {
                state.revealed = Some(key);
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
        Event::FavoriteChanged { id, favorite } => {
            for row in state.rows.iter_mut().chain(state.snapshot.items.iter_mut()) {
                if row.id == id {
                    row.favorite = favorite;
                }
            }
            state.notice = Some(if favorite {
                "Added to favorites".to_owned()
            } else {
                "Removed from favorites".to_owned()
            });
            send_quiet(state, Request::Refresh);
        }
        Event::Notice(message) => state.notice = Some(message),
        Event::AttachmentOpened { filename, preview } => {
            state.notice = Some(if preview {
                format!("Previewing {filename}")
            } else {
                format!("Saved {filename}")
            });
        }
        Event::Deleted { id, permanent } => {
            state.pending_delete = None;
            state.rows.retain(|row| row.id != id);
            state.snapshot.items.retain(|row| row.id != id);
            if state.selected.as_deref() == Some(id.as_str()) {
                state.selected = None;
                state.details = None;
                clear_reveal(state);
            }
            state.bump_rows_version();
            state.notice = Some(if permanent {
                "Item permanently deleted".to_owned()
            } else {
                "Item moved to trash".to_owned()
            });
            send_quiet(
                state,
                if state.location == Location::Trash {
                    Request::Trash
                } else {
                    Request::Refresh
                },
            );
        }
        Event::ItemRenamed { id, name } => {
            for row in state.rows.iter_mut().chain(state.snapshot.items.iter_mut()) {
                if row.id == id {
                    row.name = name.clone();
                }
            }
            state.notice = Some("Item renamed".to_owned());
            send_quiet(state, Request::Refresh);
        }
        Event::Restored { id } => {
            state.pending_delete = None;
            state.rows.retain(|row| row.id != id);
            state.snapshot.items.retain(|row| row.id != id);
            if state.location == Location::Trash {
                state.location = Location::Vault;
                state.query.clear();
            }
            if state.selected.as_deref() == Some(id.as_str()) {
                state.selected = None;
                state.details = None;
                clear_reveal(state);
            }
            state.bump_rows_version();
            state.notice = Some("Item restored".to_owned());
            send_quiet(state, Request::Refresh);
        }
        Event::Totp(meta) => {
            if state.selected.as_deref() == Some(meta.item_id.as_str()) {
                state.totp_timer = 0.0;
                state.totp = Some(meta);
            }
        }
        Event::Error(message) => state.notice = Some(message),
    }
}

pub fn submit_search(state: &mut State, _tx: &mut Tx) {
    if state.location == Location::Trash {
        return;
    }
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
    state.folders = folders_from(&state.rows);
    state.bump_rows_version();
    state.snapshot.items = state.rows.clone();
    state.selected = state.rows.first().map(|item| item.id.clone());
    state.details = Some(ItemDetails {
        id: "1".to_owned(),
        fields: vec![
            ItemField {
                key: "username".to_owned(),
                label: "Username".to_owned(),
                value: FieldValue::Text("dev@example.com".to_owned()),
            },
            ItemField {
                key: "uri-0".to_owned(),
                label: "Website".to_owned(),
                value: FieldValue::Text("https://github.com".to_owned()),
            },
            ItemField {
                key: "password".to_owned(),
                label: "Password".to_owned(),
                value: FieldValue::Masked,
            },
            ItemField {
                key: "totp".to_owned(),
                label: "TOTP".to_owned(),
                value: FieldValue::Masked,
            },
            ItemField {
                key: "created".to_owned(),
                label: "Created".to_owned(),
                value: FieldValue::Text("2026-01-01T00:00:00Z".to_owned()),
            },
            ItemField {
                key: "updated".to_owned(),
                label: "Modified".to_owned(),
                value: FieldValue::Text("2026-01-02T03:04:05Z".to_owned()),
            },
        ],
        attachments: vec![crate::backend::AttachmentMeta {
            id: "att-1".to_owned(),
            filename: "notes.txt".to_owned(),
            size: "1.2 KB".to_owned(),
        }],
    });
    state.totp = Some(TotpMeta {
        item_id: "1".to_owned(),
        ttl: 18,
        period: 30,
    });
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

pub fn scene_trash(state: &mut State) {
    scene_populated(state);
    state.location = Location::Trash;
    state.rows = vec![VaultRow {
        id: "trash-1".to_owned(),
        name: "Old recovery sheet".to_owned(),
        subtitle: String::new(),
        type_name: "note".to_owned(),
        favorite: false,
        folder: "Personal".to_owned(),
        updated: "2026-01-01T00:00:00Z".to_owned(),
    }];
    state.snapshot.items = state.rows.clone();
    state.snapshot.items_count = state.rows.len();
    state.selected = state.rows.first().map(|item| item.id.clone());
    state.bump_rows_version();
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
    fn favorite_toggle_updates_row_and_refreshes() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        set_favorite(&mut state, false);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::SetFavorite { id, favorite: false } if id == "1"
        )));

        apply(
            &mut state,
            Event::FavoriteChanged {
                id: "1".to_owned(),
                favorite: false,
            },
        );
        assert!(!state.rows[0].favorite);
        assert!(!state.snapshot.items[0].favorite);
        assert_eq!(state.notice.as_deref(), Some("Removed from favorites"));
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Refresh)));
    }

    #[test]
    fn totp_request_ticks_and_refreshes() {
        let mut state = State::default();
        scene_populated(&mut state);
        state.rows[0].id = "state-totp-test".to_owned();
        state.selected = Some("state-totp-test".to_owned());
        state.details.as_mut().unwrap().id = "state-totp-test".to_owned();
        state.totp.as_mut().unwrap().item_id = "state-totp-test".to_owned();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        let (_event_tx, event_rx) = mpsc::channel();
        state.events = Some(event_rx);
        let details = state.details.clone().unwrap();

        apply(&mut state, Event::Details(details));
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::Totp { item_id } if item_id == "state-totp-test"
        )));

        apply(
            &mut state,
            Event::Totp(TotpMeta {
                item_id: "state-totp-test".to_owned(),
                ttl: 2,
                period: 30,
            }),
        );
        let mut ui_tx = Tx::default();
        assert!(poll(&mut state, 1.0, &mut ui_tx));
        assert_eq!(state.totp.as_ref().unwrap().ttl, 1);
        assert!(poll(&mut state, 1.0, &mut ui_tx));
        assert!(state.totp.is_none());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::Totp { item_id } if item_id == "state-totp-test"
        )));
    }

    #[test]
    fn copy_field_queues_a_redacted_request_and_shows_notice() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        copy_field(&state, "password", "Password", true);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::CopyField { item_id, key, label, sensitive }
                if item_id == "1" && key == "password" && label == "Password" && sensitive
        )));

        apply(&mut state, Event::Notice("Copied Password".to_owned()));
        assert_eq!(state.notice.as_deref(), Some("Copied Password"));
    }

    #[test]
    fn edit_draft_submits_only_the_item_name() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        start_edit(&mut state);
        assert_eq!(state.edit.as_ref().unwrap().id, "1");
        state
            .edit
            .as_mut()
            .unwrap()
            .name
            .replace_text("Renamed GitHub");
        submit_edit(&mut state);
        assert!(state.edit.is_none());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::UpdateName { id, name } if id == "1" && name == "Renamed GitHub"
        )));

        apply(
            &mut state,
            Event::ItemRenamed {
                id: "1".to_owned(),
                name: "Renamed GitHub".to_owned(),
            },
        );
        assert_eq!(state.rows[0].name, "Renamed GitHub");
        assert_eq!(state.notice.as_deref(), Some("Item renamed"));
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Refresh)));
    }

    #[test]
    fn empty_edit_draft_is_rejected() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        start_edit(&mut state);
        state.edit.as_mut().unwrap().name.replace_text("   ");
        submit_edit(&mut state);
        assert!(rx.try_iter().next().is_none());
        assert_eq!(state.notice.as_deref(), Some("Enter a name for this item."));
    }

    #[test]
    fn delete_and_restore_actions_queue_requests_and_update_rows() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        delete_item(&mut state, false);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::Delete { id, permanent } if id == "1" && !permanent
        )));
        apply(
            &mut state,
            Event::Deleted {
                id: "1".to_owned(),
                permanent: false,
            },
        );
        assert!(state.rows.iter().all(|row| row.id != "1"));
        assert_eq!(state.notice.as_deref(), Some("Item moved to trash"));
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Refresh)));

        let mut state = State::default();
        scene_trash(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        delete_item(&mut state, true);
        assert!(rx.try_iter().next().is_none());
        assert_eq!(
            state.notice.as_deref(),
            Some("Click again to delete permanently")
        );
        delete_item(&mut state, true);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::Delete { id, permanent } if id == "trash-1" && permanent
        )));

        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        restore_item(&mut state);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::Restore { id } if id == "trash-1"
        )));
        apply(
            &mut state,
            Event::Restored {
                id: "trash-1".to_owned(),
            },
        );
        assert_eq!(state.location, Location::Vault);
        assert_eq!(state.notice.as_deref(), Some("Item restored"));
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Refresh)));
    }

    #[test]
    fn attachment_actions_queue_requests_and_show_notices() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        open_attachment(&mut state, "att-1".to_owned(), "notes.txt".to_owned(), true);
        open_attachment(
            &mut state,
            "att-2".to_owned(),
            "receipt.pdf".to_owned(),
            false,
        );
        let requests: Vec<_> = rx.try_iter().collect();
        assert!(requests.iter().any(|request| matches!(
            request,
            Request::Attachment { item_id, attachment_id, filename, preview }
                if item_id == "1" && attachment_id == "att-1" && filename == "notes.txt" && *preview
        )));
        assert!(requests.iter().any(|request| matches!(
            request,
            Request::Attachment { item_id, attachment_id, filename, preview }
                if item_id == "1" && attachment_id == "att-2" && filename == "receipt.pdf" && !preview
        )));

        apply(
            &mut state,
            Event::AttachmentOpened {
                filename: "notes.txt".to_owned(),
                preview: true,
            },
        );
        assert_eq!(state.notice.as_deref(), Some("Previewing notes.txt"));
        apply(
            &mut state,
            Event::AttachmentOpened {
                filename: "receipt.pdf".to_owned(),
                preview: false,
            },
        );
        assert_eq!(state.notice.as_deref(), Some("Saved receipt.pdf"));
    }

    #[test]
    fn reveal_uses_a_key_and_hide_clears_the_value() {
        let _guard = backend::REVEAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        toggle_reveal(&mut state, "password".to_owned());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::RevealField { item_id, key } if item_id == "1" && key == "password"
        )));
        backend::stash_revealed("password", "super-secret".to_owned());
        apply(
            &mut state,
            Event::Revealed {
                item_id: "1".to_owned(),
                key: "password".to_owned(),
            },
        );
        assert_eq!(state.revealed.as_deref(), Some("password"));
        assert_eq!(
            backend::revealed_value("password").as_deref(),
            Some("super-secret")
        );
        assert!(!format!("{:?}", state.revealed).contains("super-secret"));

        toggle_reveal(&mut state, "password".to_owned());
        assert_eq!(state.revealed, None);
        assert_eq!(backend::revealed_value("password"), None);
    }

    #[test]
    fn select_item_requests_details_and_clears_reveal() {
        let _guard = backend::REVEAL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        backend::stash_revealed("password", "old-secret".to_owned());
        state.revealed = Some("password".to_owned());

        select_item(&mut state, "2".to_owned());
        assert_eq!(state.selected.as_deref(), Some("2"));
        assert_eq!(state.revealed, None);
        assert_eq!(backend::revealed_value("password"), None);
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::Details { id } if id == "2"
        )));
    }

    #[test]
    fn selecting_an_item_requests_its_details() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        state.details = None;
        let mut ui_tx = Tx::default();

        assert!(move_selection(&mut state, 1, &mut ui_tx));
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Details { id } if id == "2")));

        apply(
            &mut state,
            Event::Details(ItemDetails {
                id: "1".to_owned(),
                fields: Vec::new(),
                attachments: Vec::new(),
            }),
        );
        assert!(state.details.is_none());

        let details = ItemDetails {
            id: "2".to_owned(),
            fields: vec![ItemField {
                key: "username".to_owned(),
                label: "Username".to_owned(),
                value: FieldValue::Text("ops@example.com".to_owned()),
            }],
            attachments: Vec::new(),
        };
        apply(&mut state, Event::Details(details.clone()));
        assert_eq!(state.details, Some(details));
    }

    #[test]
    fn keyboard_selection_moves_through_visible_rows() {
        let mut state = State::default();
        scene_populated(&mut state);
        let mut tx = Tx::default();
        assert_eq!(state.selected.as_deref(), Some("1"));

        assert!(move_selection(&mut state, 1, &mut tx));
        assert_eq!(state.selected.as_deref(), Some("2"));
        assert!(move_selection(&mut state, -1, &mut tx));
        assert_eq!(state.selected.as_deref(), Some("1"));

        assert!(select_last(&mut state, &mut tx));
        assert_eq!(state.selected.as_deref(), Some("4"));
        assert!(!move_selection(&mut state, 1, &mut tx));
        assert_eq!(state.selected.as_deref(), Some("4"));
        assert!(select_first(&mut state, &mut tx));
        assert_eq!(state.selected.as_deref(), Some("1"));
    }

    #[test]
    fn folder_selection_filters_rows() {
        let mut state = State::default();
        scene_populated(&mut state);
        assert_eq!(state.folders, vec!["Development", "Personal", "Work"]);

        select_folder(&mut state, "Work".to_owned());
        assert_eq!(state.visible_indices(), vec![1]);

        select_category(&mut state, Category::Logins);
        assert_eq!(state.folder, None);
        assert_eq!(state.visible_indices(), vec![0, 1]);
    }

    #[test]
    fn trash_navigation_switches_datasets() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        open_trash(&mut state);
        assert_eq!(state.location, Location::Trash);
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Trash)));
        apply(
            &mut state,
            Event::Trash(vec![VaultRow {
                id: "trash-1".to_owned(),
                folder: "Personal".to_owned(),
                ..VaultRow::default()
            }]),
        );
        assert_eq!(state.visible_indices(), vec![0]);

        select_folder(&mut state, "Work".to_owned());
        assert_eq!(state.location, Location::Vault);
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Refresh)));
        apply(&mut state, Event::Items(fixtures()));
        assert_eq!(state.visible_indices(), vec![1]);
    }

    #[test]
    fn snapshot_does_not_overwrite_open_trash() {
        let mut state = State::default();
        scene_populated(&mut state);
        state.location = Location::Trash;
        let mut snapshot = state.snapshot.clone();
        snapshot.items = vec![VaultRow {
            id: "vault-1".to_owned(),
            ..VaultRow::default()
        }];

        apply(&mut state, Event::Snapshot(snapshot));
        assert_eq!(state.location, Location::Trash);
        assert!(state.rows.iter().all(|item| item.id != "vault-1"));
    }

    #[test]
    fn poll_only_reports_visible_changes() {
        let mut state = State::default();
        scene_populated(&mut state);
        state.busy = false;
        state.notice = None;
        let (request_tx, _requests) = mpsc::channel();
        state.requests = Some(request_tx);
        let (event_tx, event_rx) = mpsc::channel();
        state.events = Some(event_rx);
        let mut tx = Tx::default();

        event_tx
            .send(Event::Notices {
                cursor: 7,
                notices: Vec::new(),
            })
            .unwrap();
        assert!(!poll(&mut state, 0.0, &mut tx));
        assert_eq!(state.event_cursor, 7);

        state.snapshot.quick_unlock_ready = true;
        event_tx
            .send(Event::Notices {
                cursor: 8,
                notices: Vec::new(),
            })
            .unwrap();
        assert!(!poll(&mut state, 0.0, &mut tx));

        event_tx
            .send(Event::Notices {
                cursor: 9,
                notices: vec![backend::Notice {
                    kind: "status".to_owned(),
                    message: "Vault unlocked".to_owned(),
                    approval_id: None,
                }],
            })
            .unwrap();
        assert!(poll(&mut state, 0.0, &mut tx));
    }

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
