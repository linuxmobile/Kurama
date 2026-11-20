use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, Receiver, Sender};

use rune::{edit::TextEdit, Tx, Waker};

use crate::backend::{
    self, clear_revealed, clear_totp, Approval, AuthOutcome, CardInput, CustomFieldInput, Event,
    FieldValue, IdentityInput, ItemDetails, ItemField, Request, Secret, SendRow, Snapshot,
    TotpMeta, VaultRow,
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

#[derive(Clone, Debug)]
pub struct SendDraft {
    pub name: TextEdit,
    pub text: TextEdit,
    pub hidden: bool,
    pub max_access: TextEdit,
    pub expiration: TextEdit,
    pub password: TextEdit,
}

#[derive(Clone, Debug)]
pub struct AttachmentView {
    pub filename: String,
    pub path: String,
    pub is_image: bool,
    pub is_text: bool,
    pub preview: bool,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum SshToolMode {
    Generate,
    Import,
    Export,
}

#[derive(Clone, Debug)]
pub struct SshToolsDraft {
    pub mode: SshToolMode,
    pub name: TextEdit,
    pub algorithm: TextEdit,
    pub comment: TextEdit,
    pub target: TextEdit,
    pub private_key: TextEdit,
    pub public_key: TextEdit,
    pub fingerprint: TextEdit,
    pub algorithm_label: String,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Location {
    Vault,
    Trash,
}

pub struct CardDraft {
    pub cardholder_name: TextEdit,
    pub number: TextEdit,
    pub brand: TextEdit,
    pub exp_month: TextEdit,
    pub exp_year: TextEdit,
    pub code: TextEdit,
}

pub struct CustomFieldDraft {
    pub name: TextEdit,
    pub value: TextEdit,
    pub field_type: i64,
}

pub struct SshDraft {
    pub private_key: TextEdit,
    pub public_key: TextEdit,
    pub fingerprint: TextEdit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthMode {
    Password,
    ApiKey,
    TwoFactor {
        provider: Option<i32>,
        providers: Vec<i32>,
    },
    NewDevice,
    Fido2 {
        status: Option<String>,
        providers: Vec<i32>,
    },
}

pub struct AuthDraft {
    pub mode: AuthMode,
    pub server: TextEdit,
    pub email: TextEdit,
    pub password: TextEdit,
    pub client_id: TextEdit,
    pub client_secret: TextEdit,
    pub code: TextEdit,
    pub new_device_code: TextEdit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorTarget {
    None,
    Create,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorOption {
    Uppercase,
    Lowercase,
    Numbers,
    Special,
    AvoidAmbiguous,
    Capitalize,
}

pub struct GeneratorDraft {
    pub passphrase: bool,
    pub length: usize,
    pub words: usize,
    pub separator: TextEdit,
    pub capitalize: bool,
    pub uppercase: bool,
    pub lowercase: bool,
    pub numbers: bool,
    pub special: bool,
    pub avoid_ambiguous: bool,
    pub value: TextEdit,
    pub entropy: f64,
    pub kind: String,
    pub target: GeneratorTarget,
}

pub struct IdentityDraft {
    pub first_name: TextEdit,
    pub last_name: TextEdit,
    pub email: TextEdit,
    pub phone: TextEdit,
    pub address1: TextEdit,
    pub city: TextEdit,
    pub postal_code: TextEdit,
    pub country: TextEdit,
}

pub struct CreateDraft {
    pub name: TextEdit,
    pub item_type: i64,
    pub favorite: bool,
    pub folder_id: Option<String>,
    pub username: TextEdit,
    pub password: TextEdit,
    pub uris: Vec<TextEdit>,
    pub notes: TextEdit,
    pub card: CardDraft,
    pub identity: IdentityDraft,
    pub ssh: SshDraft,
    pub fields: Vec<CustomFieldDraft>,
}

pub struct EditDraft {
    pub id: String,
    pub name: TextEdit,
    pub favorite: bool,
    pub folder_id: Option<String>,
    pub item_type: i64,
    pub login: bool,
    pub username: TextEdit,
    pub password: TextEdit,
    pub uris: Vec<TextEdit>,
    pub notes: TextEdit,
    pub card: CardDraft,
    pub identity: IdentityDraft,
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
    pub folder_ids: BTreeMap<String, String>,
    pub selected: Option<String>,
    pub details: Option<ItemDetails>,
    pub edit: Option<EditDraft>,
    pub create: Option<CreateDraft>,
    pub auth: Option<AuthDraft>,
    pub generator: Option<GeneratorDraft>,
    pub sends: Vec<SendRow>,
    pub sends_open: bool,
    pub send_selected: Option<String>,
    pub send_form: Option<SendDraft>,
    pub send_pending_delete: Option<String>,
    pub send_error: Option<String>,
    pub attachment_preview: Option<AttachmentView>,
    pub ssh_tools: Option<SshToolsDraft>,
    pub agent_running: bool,
    pub agent_socket: String,
    pub revealed: Option<String>,
    pub totp: Option<TotpMeta>,
    pub busy: bool,
    pub notice: Option<String>,
    pub form_error: Option<String>,
    pub approvals: Vec<Approval>,
    pub pending_delete: Option<String>,
    pub dark_override: Option<bool>,
    pub help: bool,
    rows_version: u64,
    visible_cache: RefCell<Option<VisibleCache>>,
    pub(crate) requests: Option<Sender<Request>>,
    events: Option<Receiver<Event>>,
    event_cursor: u64,
    event_timer: f64,
    agent_timer: f64,
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
            folder_ids: BTreeMap::new(),
            selected: None,
            details: None,
            edit: None,
            create: None,
            auth: None,
            generator: None,
            sends: Vec::new(),
            sends_open: false,
            send_selected: None,
            send_form: None,
            send_pending_delete: None,
            send_error: None,
            attachment_preview: None,
            ssh_tools: None,
            agent_running: false,
            agent_socket: String::new(),
            revealed: None,
            totp: None,
            busy: true,
            notice: None,
            form_error: None,
            approvals: Vec::new(),
            pending_delete: None,
            dark_override: None,
            help: false,
            rows_version: 0,
            visible_cache: RefCell::new(None),
            requests: None,
            events: None,
            event_cursor: 0,
            event_timer: 0.0,
            agent_timer: 0.0,
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

pub fn start_create() -> CreateDraft {
    CreateDraft {
        name: TextEdit::new(""),
        item_type: 1,
        favorite: false,
        folder_id: None,
        username: TextEdit::new(""),
        password: TextEdit::new("").secure(),
        uris: vec![TextEdit::new("")],
        notes: TextEdit::new(""),
        card: empty_card_draft(),
        identity: empty_identity_draft(),
        ssh: empty_ssh_draft(),
        fields: Vec::new(),
    }
}

fn optional_text(field: &TextEdit) -> Option<String> {
    let value = field.text.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn optional_secret(field: &TextEdit) -> Option<String> {
    (!field.text.is_empty()).then(|| field.text.clone())
}

fn uri_values(fields: &[TextEdit]) -> Vec<String> {
    fields.iter().filter_map(optional_text).collect()
}

fn empty_auth_draft(server: &str) -> AuthDraft {
    AuthDraft {
        mode: AuthMode::Password,
        server: TextEdit::new(server),
        email: TextEdit::new(""),
        password: TextEdit::new("").secure(),
        client_id: TextEdit::new(""),
        client_secret: TextEdit::new("").secure(),
        code: TextEdit::new(""),
        new_device_code: TextEdit::new(""),
    }
}

pub fn set_auth_mode(state: &mut State, mode: AuthMode) {
    if let Some(auth) = state.auth.as_mut() {
        auth.mode = mode;
    }
    state.form_error = None;
    state.notice = None;
}

pub fn submit_auth(state: &mut State) {
    let Some(auth) = state.auth.as_ref() else {
        return;
    };
    let server_url = auth.server.text.trim().to_owned();
    let request = if server_url.is_empty() {
        Err("Server URL is required.".to_owned())
    } else {
        match &auth.mode {
            AuthMode::Password => {
                let email = auth.email.text.trim().to_owned();
                if email.is_empty() || auth.password.text.is_empty() {
                    Err("Email and password are required.".to_owned())
                } else {
                    Ok(Request::LoginPassword {
                        server_url,
                        email,
                        password: Secret::new(auth.password.text.clone()),
                        code: None,
                        two_factor_provider: None,
                        new_device_otp: None,
                    })
                }
            }
            AuthMode::ApiKey => {
                let client_id = auth.client_id.text.trim().to_owned();
                if client_id.is_empty() || auth.client_secret.text.is_empty() {
                    Err("Client ID and secret are required.".to_owned())
                } else {
                    Ok(Request::LoginApiKey {
                        server_url,
                        client_id: Secret::new(client_id),
                        client_secret: Secret::new(auth.client_secret.text.clone()),
                    })
                }
            }
            AuthMode::TwoFactor { provider, .. } => {
                let code = auth.code.text.trim().to_owned();
                if code.is_empty() {
                    Err("Verification code is required.".to_owned())
                } else {
                    Ok(Request::LoginPassword {
                        server_url,
                        email: auth.email.text.trim().to_owned(),
                        password: Secret::new(auth.password.text.clone()),
                        code: Some(Secret::new(code)),
                        two_factor_provider: *provider,
                        new_device_otp: None,
                    })
                }
            }
            AuthMode::NewDevice => {
                let code = auth.new_device_code.text.trim().to_owned();
                if code.is_empty() {
                    Err("Verification code is required.".to_owned())
                } else {
                    Ok(Request::LoginPassword {
                        server_url,
                        email: auth.email.text.trim().to_owned(),
                        password: Secret::new(auth.password.text.clone()),
                        code: None,
                        two_factor_provider: None,
                        new_device_otp: Some(Secret::new(code)),
                    })
                }
            }
            AuthMode::Fido2 { .. } => {
                let email = auth.email.text.trim().to_owned();
                if email.is_empty() || auth.password.text.is_empty() {
                    Err("Email and password are required.".to_owned())
                } else {
                    Ok(Request::LoginPassword {
                        server_url,
                        email,
                        password: Secret::new(auth.password.text.clone()),
                        code: None,
                        two_factor_provider: Some(7),
                        new_device_otp: None,
                    })
                }
            }
        }
    };
    match request {
        Ok(request) => {
            state.form_error = None;
            state.notice = None;
            dispatch(state, request);
        }
        Err(error) => state.form_error = Some(error),
    }
}

pub fn send_two_factor_email(state: &mut State) {
    let Some(auth) = state.auth.as_ref() else {
        return;
    };
    let server_url = auth.server.text.trim().to_owned();
    let email = auth.email.text.trim().to_owned();
    let password = auth.password.text.clone();
    if server_url.is_empty() || email.is_empty() || password.is_empty() {
        state.form_error = Some("Server, email and password are required.".to_owned());
        return;
    }
    state.form_error = None;
    dispatch(
        state,
        Request::SendTwoFactorEmail {
            server_url,
            email,
            password: Secret::new(password),
        },
    );
}

pub fn submit_fido2(state: &mut State) {
    let providers = state
        .auth
        .as_ref()
        .map(|auth| match &auth.mode {
            AuthMode::Fido2 { providers, .. } => providers.clone(),
            _ => vec![7],
        })
        .unwrap_or_default();
    set_auth_mode(
        state,
        AuthMode::Fido2 {
            status: None,
            providers,
        },
    );
    submit_auth(state);
}

pub fn logout(state: &mut State) {
    state.auth = None;
    state.form_error = None;
    dispatch(state, Request::Logout);
}

fn empty_generator(target: GeneratorTarget) -> GeneratorDraft {
    GeneratorDraft {
        passphrase: false,
        length: 20,
        words: 5,
        separator: TextEdit::new("-"),
        capitalize: false,
        uppercase: true,
        lowercase: true,
        numbers: true,
        special: true,
        avoid_ambiguous: false,
        value: TextEdit::new("").secure(),
        entropy: 0.0,
        kind: "password".to_owned(),
        target,
    }
}

pub fn open_generator(state: &mut State, target: GeneratorTarget) {
    state.generator = Some(empty_generator(target));
    state.form_error = None;
}

pub fn close_generator(state: &mut State) {
    state.generator = None;
}

pub fn set_generator_passphrase(state: &mut State, passphrase: bool) {
    if let Some(generator) = state.generator.as_mut() {
        generator.passphrase = passphrase;
    }
}

pub fn adjust_generator_length(state: &mut State, delta: isize) {
    if let Some(generator) = state.generator.as_mut() {
        generator.length = generator.length.saturating_add_signed(delta).clamp(4, 128);
    }
}

pub fn adjust_generator_words(state: &mut State, delta: isize) {
    if let Some(generator) = state.generator.as_mut() {
        generator.words = generator.words.saturating_add_signed(delta).clamp(3, 12);
    }
}

pub fn toggle_generator_option(state: &mut State, option: GeneratorOption) {
    if let Some(generator) = state.generator.as_mut() {
        match option {
            GeneratorOption::Uppercase => generator.uppercase = !generator.uppercase,
            GeneratorOption::Lowercase => generator.lowercase = !generator.lowercase,
            GeneratorOption::Numbers => generator.numbers = !generator.numbers,
            GeneratorOption::Special => generator.special = !generator.special,
            GeneratorOption::AvoidAmbiguous => {
                generator.avoid_ambiguous = !generator.avoid_ambiguous
            }
            GeneratorOption::Capitalize => generator.capitalize = !generator.capitalize,
        }
    }
}

pub fn generate_password(state: &mut State) {
    let Some(generator) = state.generator.as_ref() else {
        return;
    };
    let request = Request::GeneratePassword {
        passphrase: generator.passphrase,
        length: generator.length,
        words: generator.words,
        separator: generator.separator.text.clone(),
        capitalize: generator.capitalize,
        uppercase: generator.uppercase,
        lowercase: generator.lowercase,
        numbers: generator.numbers,
        special: generator.special,
        avoid_ambiguous: generator.avoid_ambiguous,
    };
    state.form_error = None;
    dispatch(state, request);
}

pub fn copy_generated(state: &mut State) {
    let Some(generator) = state.generator.as_ref() else {
        return;
    };
    let value = generator.value.text.clone();
    if value.is_empty() {
        state.form_error = Some("Generate a value first.".to_owned());
        return;
    }
    let label = if generator.passphrase {
        "Passphrase"
    } else {
        "Password"
    }
    .to_owned();
    state.form_error = None;
    dispatch(
        state,
        Request::CopyText {
            text: Secret::new(value),
            label,
        },
    );
}

pub fn use_generated(state: &mut State) {
    let Some(generator) = state.generator.as_ref() else {
        return;
    };
    let value = generator.value.text.clone();
    let target = generator.target;
    if value.is_empty() {
        state.form_error = Some("Generate a value first.".to_owned());
        return;
    }
    match target {
        GeneratorTarget::Create => {
            if let Some(draft) = state.create.as_mut() {
                draft.password.replace_text(&value);
            }
        }
        GeneratorTarget::Edit => {
            if let Some(draft) = state.edit.as_mut() {
                draft.password.replace_text(&value);
            }
        }
        GeneratorTarget::None => {}
    }
    state.generator = None;
    state.form_error = None;
    state.notice = Some("Generated value inserted.".to_owned());
}

fn prefilled_uri_draft(state: &State, item_id: &str) -> Vec<TextEdit> {
    let uris: BTreeMap<usize, String> = state
        .details
        .iter()
        .filter(|details| details.id == item_id)
        .flat_map(|details| details.fields.iter())
        .filter_map(|field| {
            let index = field.key.strip_prefix("uri-")?.parse::<usize>().ok()?;
            let FieldValue::Text(value) = &field.value else {
                return None;
            };
            Some((index, value.clone()))
        })
        .collect();
    let values: Vec<_> = uris.into_values().collect();
    if values.is_empty() {
        vec![TextEdit::new("")]
    } else {
        values.into_iter().map(TextEdit::new).collect()
    }
}

fn empty_card_draft() -> CardDraft {
    CardDraft {
        cardholder_name: TextEdit::new(""),
        number: TextEdit::new("").secure(),
        brand: TextEdit::new(""),
        exp_month: TextEdit::new(""),
        exp_year: TextEdit::new(""),
        code: TextEdit::new("").secure(),
    }
}

fn card_input(draft: &CardDraft) -> Option<CardInput> {
    let cardholder_name = optional_secret(&draft.cardholder_name).map(Secret::new);
    let number = optional_secret(&draft.number).map(Secret::new);
    let brand = optional_secret(&draft.brand).map(Secret::new);
    let exp_month = optional_secret(&draft.exp_month).map(Secret::new);
    let exp_year = optional_secret(&draft.exp_year).map(Secret::new);
    let code = optional_secret(&draft.code).map(Secret::new);
    let used = [
        &cardholder_name,
        &number,
        &brand,
        &exp_month,
        &exp_year,
        &code,
    ]
    .iter()
    .any(|field| field.is_some());
    used.then_some(CardInput {
        cardholder_name,
        number,
        brand,
        exp_month,
        exp_year,
        code,
    })
}

fn empty_identity_draft() -> IdentityDraft {
    IdentityDraft {
        first_name: TextEdit::new(""),
        last_name: TextEdit::new(""),
        email: TextEdit::new(""),
        phone: TextEdit::new(""),
        address1: TextEdit::new(""),
        city: TextEdit::new(""),
        postal_code: TextEdit::new(""),
        country: TextEdit::new(""),
    }
}

fn empty_ssh_draft() -> SshDraft {
    SshDraft {
        private_key: TextEdit::new("").secure(),
        public_key: TextEdit::new(""),
        fingerprint: TextEdit::new(""),
    }
}

fn identity_input(draft: &IdentityDraft) -> Option<IdentityInput> {
    let first_name = optional_secret(&draft.first_name).map(Secret::new);
    let last_name = optional_secret(&draft.last_name).map(Secret::new);
    let email = optional_secret(&draft.email).map(Secret::new);
    let phone = optional_secret(&draft.phone).map(Secret::new);
    let address1 = optional_secret(&draft.address1).map(Secret::new);
    let city = optional_secret(&draft.city).map(Secret::new);
    let postal_code = optional_secret(&draft.postal_code).map(Secret::new);
    let country = optional_secret(&draft.country).map(Secret::new);
    let used = [
        &first_name,
        &last_name,
        &email,
        &phone,
        &address1,
        &city,
        &postal_code,
        &country,
    ]
    .iter()
    .any(|field| field.is_some());
    used.then_some(IdentityInput {
        first_name,
        last_name,
        email,
        phone,
        address1,
        city,
        postal_code,
        country,
    })
}

fn detail_text(state: &State, key: &str) -> String {
    state
        .details
        .as_ref()
        .and_then(|details| details.fields.iter().find(|field| field.key == key))
        .and_then(|field| match &field.value {
            FieldValue::Text(value) => Some(value.clone()),
            FieldValue::Masked => None,
        })
        .unwrap_or_default()
}

fn split_name(value: &str) -> (&str, &str) {
    value.split_once(' ').unwrap_or((value, ""))
}

fn prefilled_identity_draft(state: &State, fallback: &str) -> IdentityDraft {
    let full_name = detail_text(state, "name");
    let source = if full_name.is_empty() {
        fallback
    } else {
        &full_name
    };
    let (first, last) = split_name(source);
    IdentityDraft {
        first_name: TextEdit::new(first),
        last_name: TextEdit::new(last),
        email: TextEdit::new(detail_text(state, "email")),
        phone: TextEdit::new(detail_text(state, "phone")),
        address1: TextEdit::new(""),
        city: TextEdit::new(""),
        postal_code: TextEdit::new(""),
        country: TextEdit::new(""),
    }
}

fn item_type_id(type_name: &str) -> i64 {
    match type_name {
        "login" => 1,
        "note" => 2,
        "card" => 3,
        "identity" => 4,
        "ssh_key" => 5,
        _ => 1,
    }
}

pub fn toggle_create_favorite(state: &mut State) {
    if let Some(draft) = state.create.as_mut() {
        draft.favorite = !draft.favorite;
    }
}

fn stepped_folder(
    folders: &[String],
    ids: &BTreeMap<String, String>,
    current: &Option<String>,
    delta: isize,
) -> Option<String> {
    if folders.is_empty() {
        return None;
    }
    let index = current
        .as_ref()
        .and_then(|id| folders.iter().position(|name| ids.get(name) == Some(id)))
        .map(|index| index as isize + 1)
        .unwrap_or(0);
    let len = folders.len() as isize + 1;
    let next = (index + delta).rem_euclid(len);
    if next == 0 {
        None
    } else {
        ids.get(&folders[next as usize - 1]).cloned()
    }
}

pub fn folder_label(state: &State, folder_id: Option<&str>) -> String {
    folder_id
        .and_then(|id| {
            state
                .folders
                .iter()
                .find(|name| state.folder_ids.get(*name).map(String::as_str) == Some(id))
        })
        .cloned()
        .unwrap_or_else(|| "None".to_owned())
}

pub fn cycle_create_folder(state: &mut State, delta: isize) {
    let Some(current) = state.create.as_ref().map(|draft| draft.folder_id.clone()) else {
        return;
    };
    let next = stepped_folder(&state.folders, &state.folder_ids, &current, delta);
    if let Some(draft) = state.create.as_mut() {
        draft.folder_id = next;
    }
}

pub fn set_create_type(state: &mut State, item_type: i64) {
    if let Some(draft) = state.create.as_mut() {
        draft.item_type = item_type;
    }
}

fn add_create_field_raw(draft: &mut CreateDraft) {
    draft.fields.push(CustomFieldDraft {
        name: TextEdit::new("Recovery phrase"),
        value: TextEdit::new("correct horse battery staple"),
        field_type: 1,
    });
}

pub fn add_create_field(state: &mut State) {
    if let Some(draft) = state.create.as_mut() {
        draft.fields.push(CustomFieldDraft {
            name: TextEdit::new(""),
            value: TextEdit::new(""),
            field_type: 0,
        });
    }
}

pub fn add_create_uri(state: &mut State) {
    if let Some(draft) = state.create.as_mut() {
        draft.uris.push(TextEdit::new(""));
    }
}

pub fn remove_create_uri(state: &mut State, index: usize) {
    if let Some(draft) = state.create.as_mut() {
        if index < draft.uris.len() {
            draft.uris.remove(index);
        }
    }
}

pub fn add_edit_uri(state: &mut State) {
    if let Some(draft) = state.edit.as_mut() {
        draft.uris.push(TextEdit::new(""));
    }
}

pub fn remove_edit_uri(state: &mut State, index: usize) {
    if let Some(draft) = state.edit.as_mut() {
        if index < draft.uris.len() {
            draft.uris.remove(index);
        }
    }
}

pub fn remove_create_field(state: &mut State, index: usize) {
    if let Some(draft) = state.create.as_mut() {
        if index < draft.fields.len() {
            draft.fields.remove(index);
        }
    }
}

pub fn cycle_create_field_type(state: &mut State, index: usize) {
    if let Some(field) = state
        .create
        .as_mut()
        .and_then(|draft| draft.fields.get_mut(index))
    {
        field.field_type = (field.field_type + 1) % 3;
    }
}

pub fn toggle_create_field_value(state: &mut State, index: usize) {
    if let Some(field) = state
        .create
        .as_mut()
        .and_then(|draft| draft.fields.get_mut(index))
    {
        let enabled = field.value.text.eq_ignore_ascii_case("true") || field.value.text == "1";
        field.value = TextEdit::new(if enabled { "false" } else { "true" });
    }
}

fn custom_fields_input(drafts: &[CustomFieldDraft]) -> Vec<CustomFieldInput> {
    drafts
        .iter()
        .filter(|field| !field.name.text.trim().is_empty())
        .map(|field| CustomFieldInput {
            name: field.name.text.trim().to_owned(),
            value: Secret::new(field.value.text.clone()),
            field_type: field.field_type,
        })
        .collect()
}

pub fn submit_create(state: &mut State) {
    let Some(draft) = state.create.take() else {
        return;
    };
    let name = draft.name.text.trim().to_owned();
    if name.is_empty() {
        state.form_error = Some("Item name is required.".to_owned());
        state.create = Some(draft);
        return;
    }
    if draft.item_type == 5 && draft.ssh.private_key.text.trim().is_empty() {
        state.form_error = Some("SSH private key is required.".to_owned());
        state.create = Some(draft);
        return;
    }
    if draft
        .fields
        .iter()
        .any(|field| field.name.text.trim().is_empty() && !field.value.text.trim().is_empty())
    {
        state.form_error = Some("Name every custom field that has a value.".to_owned());
        state.create = Some(draft);
        return;
    }
    state.form_error = None;
    if draft.item_type == 5 {
        let private_key = Secret::new(draft.ssh.private_key.text.clone());
        dispatch(
            state,
            Request::CreateSshKey {
                name,
                private_key,
                public_key: optional_text(&draft.ssh.public_key),
                fingerprint: optional_text(&draft.ssh.fingerprint),
                notes: optional_secret(&draft.notes).map(Secret::new),
                folder_id: draft.folder_id,
                favorite: draft.favorite,
            },
        );
        return;
    }
    dispatch(
        state,
        Request::CreateItem {
            name,
            item_type: draft.item_type,
            favorite: draft.favorite,
            folder_id: draft.folder_id,
            username: optional_text(&draft.username),
            password: optional_secret(&draft.password).map(Secret::new),
            uris: uri_values(&draft.uris),
            notes: optional_secret(&draft.notes).map(Secret::new),
            card: (draft.item_type == 3)
                .then(|| card_input(&draft.card))
                .flatten(),
            identity: (draft.item_type == 4)
                .then(|| identity_input(&draft.identity))
                .flatten(),
            fields: custom_fields_input(&draft.fields),
        },
    );
}

pub fn cancel_create(state: &mut State) {
    state.create = None;
    state.form_error = None;
}

pub fn start_edit(state: &mut State) {
    let Some(item) = state.selected_item() else {
        return;
    };
    state.edit = Some(EditDraft {
        id: item.id.clone(),
        name: TextEdit::new(&item.name),
        favorite: item.favorite,
        folder_id: (!item.folder_id.is_empty()).then(|| item.folder_id.clone()),
        item_type: item_type_id(&item.type_name),
        login: item.type_name == "login",
        username: TextEdit::new(&item.subtitle),
        password: TextEdit::new("").secure(),
        uris: prefilled_uri_draft(state, &item.id),
        notes: TextEdit::new(""),
        card: empty_card_draft(),
        identity: if item.type_name == "identity" {
            prefilled_identity_draft(state, &item.subtitle)
        } else {
            empty_identity_draft()
        },
    });
    state.form_error = None;
}

pub fn toggle_edit_favorite(state: &mut State) {
    if let Some(draft) = state.edit.as_mut() {
        draft.favorite = !draft.favorite;
    }
}

pub fn cycle_edit_folder(state: &mut State, delta: isize) {
    let Some(current) = state.edit.as_ref().map(|draft| draft.folder_id.clone()) else {
        return;
    };
    let next = stepped_folder(&state.folders, &state.folder_ids, &current, delta);
    if let Some(draft) = state.edit.as_mut() {
        draft.folder_id = next;
    }
}

pub fn submit_edit(state: &mut State) {
    let Some(draft) = state.edit.take() else {
        return;
    };
    let name = draft.name.text.trim().to_owned();
    if name.is_empty() {
        state.form_error = Some("Item name is required.".to_owned());
        state.edit = Some(draft);
        return;
    }
    state.form_error = None;
    dispatch(
        state,
        Request::UpdateItem {
            id: draft.id,
            name,
            favorite: draft.favorite,
            folder_id: draft.folder_id,
            username: optional_text(&draft.username),
            password: optional_secret(&draft.password).map(Secret::new),
            uris: uri_values(&draft.uris),
            notes: optional_secret(&draft.notes).map(Secret::new),
            card: (draft.item_type == 3)
                .then(|| card_input(&draft.card))
                .flatten(),
            identity: (draft.item_type == 4)
                .then(|| identity_input(&draft.identity))
                .flatten(),
        },
    );
}

pub fn cancel_edit(state: &mut State) {
    state.edit = None;
    state.form_error = None;
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
    let _ = request_tx.send(Request::Folders);
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
        send_quiet(state, Request::Approvals);
        state.agent_timer += 1.0;
        if state.agent_timer >= 5.0 {
            state.agent_timer = 0.0;
            send_quiet(state, Request::AgentStatus);
        }
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
    state.help.hash(&mut hasher);
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
    state.sends_open.hash(&mut hasher);
    state.send_selected.hash(&mut hasher);
    state.send_pending_delete.hash(&mut hasher);
    state.send_error.hash(&mut hasher);
    state
        .attachment_preview
        .as_ref()
        .map(|view| view.filename.as_str())
        .hash(&mut hasher);
    state
        .ssh_tools
        .as_ref()
        .map(|draft| draft.mode)
        .hash(&mut hasher);
    state
        .ssh_tools
        .as_ref()
        .map(|draft| draft.fingerprint.text.as_str())
        .hash(&mut hasher);
    state.agent_running.hash(&mut hasher);
    state.agent_socket.hash(&mut hasher);
    for send in &state.sends {
        send.id.hash(&mut hasher);
        send.name.hash(&mut hasher);
        send.disabled.hash(&mut hasher);
    }
    state
        .send_form
        .as_ref()
        .map(|draft| draft.hidden)
        .hash(&mut hasher);
    state
        .edit
        .as_ref()
        .map(|draft| draft.id.as_str())
        .hash(&mut hasher);
    state
        .edit
        .as_ref()
        .map(|draft| draft.favorite)
        .hash(&mut hasher);
    state
        .edit
        .as_ref()
        .and_then(|draft| draft.folder_id.as_deref())
        .hash(&mut hasher);
    state
        .create
        .as_ref()
        .map(|draft| draft.item_type)
        .hash(&mut hasher);
    state
        .create
        .as_ref()
        .map(|draft| draft.favorite)
        .hash(&mut hasher);
    state
        .create
        .as_ref()
        .and_then(|draft| draft.folder_id.as_deref())
        .hash(&mut hasher);
    state
        .edit
        .as_ref()
        .map(|draft| draft.login)
        .hash(&mut hasher);
    hasher.finish()
}

fn apply_auth_outcome(state: &mut State, outcome: AuthOutcome) {
    let fido2 = outcome.fido2_status.is_some() || outcome.two_factor_provider == Some(7);
    if !outcome.ok {
        state.form_error = None;
        if fido2 {
            if let Some(auth) = state.auth.as_mut() {
                auth.mode = AuthMode::Fido2 {
                    status: outcome.fido2_status.clone(),
                    providers: outcome.two_factor_providers.clone(),
                };
            }
        }
        state.notice = outcome.error;
        return;
    }
    state.form_error = None;
    if outcome.new_device_verification_required {
        state.notice = None;
        if let Some(auth) = state.auth.as_mut() {
            auth.mode = AuthMode::NewDevice;
        }
        return;
    }
    if fido2 {
        if let Some(auth) = state.auth.as_mut() {
            auth.mode = AuthMode::Fido2 {
                status: outcome.fido2_status.clone(),
                providers: outcome.two_factor_providers.clone(),
            };
        }
        state.notice = outcome.fido2_status;
        return;
    }
    state.notice = None;
    if outcome.two_factor_required {
        if let Some(auth) = state.auth.as_mut() {
            auth.mode = AuthMode::TwoFactor {
                provider: outcome
                    .two_factor_provider
                    .or_else(|| outcome.two_factor_providers.first().copied()),
                providers: outcome.two_factor_providers,
            };
        }
        return;
    }
    dispatch(state, Request::Refresh);
}

fn is_session_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("session expired")
        || lower.contains("no active session")
        || lower.contains("log in again")
        || lower.contains("log in first")
        || lower.contains("unauthorized")
        || lower.contains("401")
        || lower.contains("403")
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
                state.create = None;
                state.attachment_preview = None;
                backend::clear_attachment_text();
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
            if state.snapshot.status == "unauthenticated" {
                if state.auth.is_none() {
                    let server = state.snapshot.server.clone();
                    state.auth = Some(empty_auth_draft(&server));
                }
            } else {
                state.auth = None;
            }
        }
        Event::Auth(outcome) => apply_auth_outcome(state, outcome),
        Event::Generated {
            value,
            entropy,
            kind,
        } => {
            if let Some(generator) = state.generator.as_mut() {
                generator.value.replace_text(value.as_str());
                generator.entropy = entropy;
                generator.kind = kind;
            }
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
        Event::Sends(rows) => {
            backend::clear_send_urls();
            for row in &rows {
                backend::stash_send_url(&row.id, &row.url);
            }
            state.sends = rows;
            state.send_error = None;
            let selected_missing = state
                .send_selected
                .as_ref()
                .is_some_and(|id| !state.sends.iter().any(|row| &row.id == id));
            if selected_missing || state.send_selected.is_none() {
                state.send_selected = state.sends.first().map(|row| row.id.clone());
            }
        }
        Event::SendCreated { id, url } => {
            backend::stash_send_url(&id, &url);
            let link = Secret::new(url.as_str().to_owned());
            state.send_form = None;
            state.form_error = None;
            state.send_selected = Some(id);
            state.notice = Some("Send created · link copied".to_owned());
            dispatch(
                state,
                Request::CopyText {
                    text: link,
                    label: "Send link".to_owned(),
                },
            );
            dispatch(state, Request::ListSends);
        }
        Event::SendChanged { id, disabled } => {
            if let Some(row) = state.sends.iter_mut().find(|row| row.id == id) {
                row.disabled = disabled;
            }
            state.notice = Some(if disabled {
                "Send disabled".to_owned()
            } else {
                "Send updated".to_owned()
            });
            dispatch(state, Request::ListSends);
        }
        Event::SendDeleted { id } => {
            state.sends.retain(|row| row.id != id);
            backend::clear_send_urls();
            if state.send_selected.as_deref() == Some(id.as_str()) {
                state.send_selected = state.sends.first().map(|row| row.id.clone());
            }
            state.send_pending_delete = None;
            state.notice = Some("Send deleted".to_owned());
            dispatch(state, Request::ListSends);
        }
        Event::SshKeyGenerated {
            private,
            public,
            fingerprint,
            algorithm,
        } => {
            if let Some(draft) = state.ssh_tools.as_mut() {
                draft.private_key.replace_text(private.as_str());
                draft.public_key.replace_text(&public);
                draft.fingerprint.replace_text(&fingerprint);
                draft.algorithm_label = algorithm;
            }
            state.notice = Some("SSH key generated".to_owned());
        }
        Event::SshKeyImported {
            public,
            fingerprint,
        } => {
            if let Some(draft) = state.ssh_tools.as_mut() {
                draft.public_key.replace_text(&public);
                draft.fingerprint.replace_text(&fingerprint);
            }
            state.notice = Some("SSH key imported".to_owned());
        }
        Event::SshKeyExported {
            private,
            public,
            fingerprint,
        } => {
            if let Some(draft) = state.ssh_tools.as_mut() {
                draft.mode = SshToolMode::Export;
                draft.private_key.replace_text(private.as_str());
                draft.public_key.replace_text(&public);
                draft.fingerprint.replace_text(&fingerprint);
            }
            state.notice = Some("SSH key exported".to_owned());
        }
        Event::AgentStatus { running, socket } => {
            state.agent_running = running;
            state.agent_socket = socket;
        }
        Event::ApprovalAnswered { id, approve } => {
            state.approvals.retain(|prompt| prompt.id != id);
            state.notice = Some(if approve {
                "SSH request approved".to_owned()
            } else {
                "SSH request denied".to_owned()
            });
            send_quiet(state, Request::Approvals);
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
        Event::AttachmentOpened { preview, result } => {
            backend::clear_attachment_text();
            if let Some(text) = result.text {
                backend::stash_attachment_text(&result.filename, text);
            }
            let filename = result.filename;
            state.attachment_preview = Some(AttachmentView {
                filename: filename.clone(),
                path: result.path,
                is_image: result.is_image,
                is_text: result.is_text,
                preview,
            });
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
                state.edit = None;
                state.create = None;
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
        Event::ItemCreated { id, name } => {
            if state.location == Location::Trash {
                state.location = Location::Vault;
                state.query.clear();
            }
            state.selected = if id.is_empty() { None } else { Some(id) };
            state.details = None;
            clear_reveal(state);
            state.notice = format!("Created {name}").into();
            send_quiet(state, Request::Refresh);
        }
        Event::ItemRenamed { id, name, favorite } => {
            for row in state.rows.iter_mut().chain(state.snapshot.items.iter_mut()) {
                if row.id == id {
                    row.name = name.clone();
                    row.favorite = favorite;
                }
            }
            state.notice = Some("Item renamed".to_owned());
            send_quiet(state, Request::Refresh);
        }
        Event::Folders(folders) => {
            state.folders = folders.iter().map(|folder| folder.name.clone()).collect();
            state.folder_ids = folders
                .into_iter()
                .map(|folder| (folder.name, folder.id))
                .collect();
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
        Event::Error(message) => {
            if is_session_error(&message) {
                state.snapshot.status = "unauthenticated".to_owned();
                state.rows.clear();
                state.folders.clear();
                state.folder_ids.clear();
                state.location = Location::Vault;
                state.folder = None;
                state.selected = None;
                state.details = None;
                state.edit = None;
                state.create = None;
                clear_reveal(state);
                state.password.replace_text("");
                state.bump_rows_version();
                if state.auth.is_none() {
                    let server = state.snapshot.server.clone();
                    state.auth = Some(empty_auth_draft(&server));
                }
                state.notice = Some("Your session expired. Sign in again.".to_owned());
            } else {
                state.notice = Some(message);
            }
        }
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

fn empty_send_draft() -> SendDraft {
    SendDraft {
        name: TextEdit::new(""),
        text: TextEdit::new("").secure(),
        hidden: false,
        max_access: TextEdit::new(""),
        expiration: TextEdit::new(""),
        password: TextEdit::new("").secure(),
    }
}

pub fn open_sends(state: &mut State) {
    state.sends_open = true;
    state.send_error = None;
    dispatch(state, Request::ListSends);
}

pub fn close_sends(state: &mut State) {
    state.sends_open = false;
    state.send_form = None;
    state.send_pending_delete = None;
    state.send_error = None;
}

pub fn new_send(state: &mut State) {
    state.send_form = Some(empty_send_draft());
    state.form_error = None;
}

pub fn cancel_send(state: &mut State) {
    state.send_form = None;
    state.form_error = None;
}

pub fn toggle_send_hidden(state: &mut State) {
    if let Some(draft) = state.send_form.as_mut() {
        draft.hidden = !draft.hidden;
    }
}

pub fn select_send(state: &mut State, id: String) {
    state.send_selected = Some(id);
}

pub fn submit_send(state: &mut State) {
    let Some(draft) = state.send_form.as_ref() else {
        return;
    };
    let text = draft.text.text.clone();
    if text.trim().is_empty() {
        state.form_error = Some("Enter the text to share.".to_owned());
        return;
    }
    let max_access = draft.max_access.text.trim();
    let max_access = if max_access.is_empty() {
        None
    } else {
        match max_access.parse::<u32>() {
            Ok(value) if value > 0 => Some(value),
            _ => {
                state.form_error = Some("Maximum accesses must be a positive number.".to_owned());
                return;
            }
        }
    };
    let name = {
        let name = draft.name.text.trim();
        if name.is_empty() {
            "Untitled send".to_owned()
        } else {
            name.to_owned()
        }
    };
    let expiration = draft.expiration.text.trim();
    let expiration = (!expiration.is_empty()).then(|| expiration.to_owned());
    let password = draft.password.text.trim();
    let password = (!password.is_empty()).then(|| Secret::new(password.to_owned()));
    let hidden = draft.hidden;
    state.form_error = None;
    dispatch(
        state,
        Request::CreateSend {
            name,
            text: Secret::new(text),
            hidden,
            max_access,
            expiration,
            password,
        },
    );
}

pub fn copy_send(state: &mut State, id: String) {
    let Some(url) = backend::send_url(&id) else {
        state.notice = Some("This send has no access link.".to_owned());
        return;
    };
    dispatch(
        state,
        Request::CopyText {
            text: Secret::new(url),
            label: "Send link".to_owned(),
        },
    );
}

pub fn disable_send(state: &mut State, id: String) {
    dispatch(state, Request::DisableSend { id });
}

pub fn request_delete_send(state: &mut State, id: String) {
    state.send_pending_delete = Some(id);
}

pub fn cancel_delete_send(state: &mut State) {
    state.send_pending_delete = None;
}

pub fn confirm_delete_send(state: &mut State) {
    let Some(id) = state.send_pending_delete.take() else {
        return;
    };
    dispatch(state, Request::DeleteSend { id });
}

pub fn close_attachment(state: &mut State) {
    state.attachment_preview = None;
    backend::clear_attachment_text();
}

const SSH_ALGORITHMS: [&str; 5] = [
    "ed25519",
    "rsa-2048",
    "rsa-4096",
    "ecdsa-p256",
    "ecdsa-p384",
];

fn empty_ssh_tools_draft() -> SshToolsDraft {
    SshToolsDraft {
        mode: SshToolMode::Generate,
        name: TextEdit::new(""),
        algorithm: TextEdit::new(SSH_ALGORITHMS[0]),
        comment: TextEdit::new(""),
        target: TextEdit::new(""),
        private_key: TextEdit::new("").secure(),
        public_key: TextEdit::new(""),
        fingerprint: TextEdit::new(""),
        algorithm_label: "ED25519".to_owned(),
    }
}

pub fn open_ssh_tools(state: &mut State) {
    state.ssh_tools = Some(empty_ssh_tools_draft());
    state.attachment_preview = None;
    state.form_error = None;
    dispatch(state, Request::AgentStatus);
    dispatch(state, Request::Approvals);
}

pub fn close_ssh_tools(state: &mut State) {
    state.ssh_tools = None;
    state.form_error = None;
}

pub fn refresh_agent_status(state: &mut State) {
    dispatch(state, Request::AgentStatus);
}

pub fn apply_agent_settings(state: &mut State) {
    dispatch(state, Request::AgentApply);
}

pub fn answer_approval(state: &mut State, id: u64, approve: bool) {
    state.approvals.retain(|prompt| prompt.id != id);
    dispatch(state, Request::AgentReply { id, approve });
}

pub fn set_ssh_tool_mode(state: &mut State, mode: SshToolMode) {
    if let Some(draft) = state.ssh_tools.as_mut() {
        draft.mode = mode;
    }
    state.form_error = None;
}

pub fn cycle_ssh_algorithm(state: &mut State, delta: isize) {
    let Some(draft) = state.ssh_tools.as_mut() else {
        return;
    };
    let current = SSH_ALGORITHMS
        .iter()
        .position(|algorithm| *algorithm == draft.algorithm.text)
        .unwrap_or(0);
    let len = SSH_ALGORITHMS.len() as isize;
    let next = (current as isize + delta).rem_euclid(len) as usize;
    draft.algorithm.replace_text(SSH_ALGORITHMS[next]);
}

pub fn submit_ssh_generate(state: &mut State) {
    let Some(draft) = state.ssh_tools.as_ref() else {
        return;
    };
    dispatch(
        state,
        Request::GenerateSshKey {
            algorithm: draft.algorithm.text.trim().to_owned(),
            comment: draft.comment.text.trim().to_owned(),
        },
    );
}

pub fn submit_ssh_import(state: &mut State) {
    let Some(draft) = state.ssh_tools.as_ref() else {
        return;
    };
    let private_key = draft.private_key.text.trim();
    if private_key.is_empty() {
        state.form_error = Some("Paste an OpenSSH private key to import.".to_owned());
        return;
    }
    state.form_error = None;
    dispatch(
        state,
        Request::ImportSshKey {
            private_key: Secret::new(private_key.to_owned()),
        },
    );
}

pub fn submit_ssh_export(state: &mut State) {
    let Some(draft) = state.ssh_tools.as_ref() else {
        return;
    };
    let item_id = draft.target.text.trim();
    if item_id.is_empty() {
        state.form_error = Some("Enter the SSH key item name or ID to export.".to_owned());
        return;
    }
    state.form_error = None;
    dispatch(
        state,
        Request::ExportSshKey {
            item_id: item_id.to_owned(),
        },
    );
}

pub fn save_ssh_tool_key(state: &mut State) {
    let Some(draft) = state.ssh_tools.as_ref() else {
        return;
    };
    if draft.private_key.text.trim().is_empty() || draft.public_key.text.trim().is_empty() {
        state.form_error = Some("Generate, import, or export a key before saving it.".to_owned());
        return;
    }
    let name = {
        let name = draft.name.text.trim();
        if name.is_empty() {
            "SSH key".to_owned()
        } else {
            name.to_owned()
        }
    };
    let private_key = draft.private_key.text.trim().to_owned();
    let public_key = draft.public_key.text.trim().to_owned();
    let fingerprint = draft.fingerprint.text.trim().to_owned();
    state.form_error = None;
    dispatch(
        state,
        Request::CreateSshKey {
            name,
            private_key: Secret::new(private_key),
            public_key: Some(public_key),
            fingerprint: Some(fingerprint),
            notes: None,
            folder_id: None,
            favorite: false,
        },
    );
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
    state.folder_ids = state
        .folders
        .iter()
        .map(|name| (name.clone(), name.to_lowercase()))
        .collect();
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
                key: "uri-1".to_owned(),
                label: "Website".to_owned(),
                value: FieldValue::Text("https://docs.example.com".to_owned()),
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
            folder_id: String::new(),
            updated: "2026-01-02T03:04:05Z".to_owned(),
        })
        .collect();
    state.snapshot.items = state.rows.clone();
    state.snapshot.items_count = state.rows.len();
    state.selected = state.rows.first().map(|item| item.id.clone());
    state.bump_rows_version();
    state.busy = false;
}

pub fn scene_account(state: &mut State) {
    state.snapshot = Snapshot {
        status: "unauthenticated".to_owned(),
        server: "https://vault.example.com".to_owned(),
        ..Snapshot::default()
    };
    state.auth = Some(empty_auth_draft(&state.snapshot.server));
    state.form_error = None;
    state.notice = None;
    state.busy = false;
}

pub fn scene_disconnected(state: &mut State) {
    state.busy = false;
    state.notice = Some("Kurama backend is not running.".to_owned());
}

pub fn scene_account_two_factor(state: &mut State) {
    scene_account(state);
    if let Some(auth) = state.auth.as_mut() {
        auth.mode = AuthMode::TwoFactor {
            provider: Some(0),
            providers: vec![0],
        };
        auth.email.replace_text("user@example.com");
        auth.password.replace_text("master-secret");
    }
}

pub fn scene_session_expired(state: &mut State) {
    scene_populated(state);
    apply(
        state,
        Event::Error("Session expired or credentials rejected. Please log in again.".to_owned()),
    );
}

pub fn scene_account_fido2(state: &mut State) {
    scene_account(state);
    if let Some(auth) = state.auth.as_mut() {
        auth.mode = AuthMode::Fido2 {
            status: Some("Security key detected".to_owned()),
            providers: vec![7],
        };
        auth.email.replace_text("user@example.com");
        auth.password.replace_text("master-secret");
    }
}

pub fn scene_generator(state: &mut State) {
    scene_populated(state);
    open_generator(state, GeneratorTarget::None);
    state
        .generator
        .as_mut()
        .unwrap()
        .value
        .replace_text("Co4rect-Horse-Battery");
    state.generator.as_mut().unwrap().entropy = 78.4;
}

pub fn scene_sends(state: &mut State) {
    scene_populated(state);
    state.sends_open = true;
    state.sends = vec![
        SendRow {
            id: "send-1".to_owned(),
            name: "Notes for Alice".to_owned(),
            disabled: false,
            expiration: "2026-02-01T00:00:00Z".to_owned(),
            max_access: 5,
            access_count: 2,
            url: Secret::new("https://vault.example.com/send#abc123".to_owned()),
        },
        SendRow {
            id: "send-2".to_owned(),
            name: "Archived report".to_owned(),
            disabled: true,
            expiration: String::new(),
            max_access: 0,
            access_count: 0,
            url: Secret::new(String::new()),
        },
    ];
    backend::stash_send_url("send-1", &state.sends[0].url);
    backend::stash_send_url("send-2", &state.sends[1].url);
    state.send_selected = Some("send-1".to_owned());
}

pub fn scene_send_form(state: &mut State) {
    scene_sends(state);
    state.send_form = Some(empty_send_draft());
    state
        .send_form
        .as_mut()
        .unwrap()
        .text
        .replace_text("Share this text securely");
    state.form_error = None;
}

pub fn scene_attachment_preview(state: &mut State) {
    scene_populated(state);
    state.attachment_preview = Some(AttachmentView {
        filename: "notes.txt".to_owned(),
        path: "/tmp/kurama/attachments/1/att-1/notes.txt".to_owned(),
        is_image: false,
        is_text: true,
        preview: true,
    });
    backend::stash_attachment_text(
        "notes.txt",
        Secret::new("Meeting notes\n- review phase 8\n- ship release build".to_owned()),
    );
}

pub fn scene_ssh_tools(state: &mut State) {
    scene_populated(state);
    let mut draft = empty_ssh_tools_draft();
    draft.private_key.replace_text("-----BEGIN OPENSSH PRIVATE KEY-----\nprivate-key-material\n-----END OPENSSH PRIVATE KEY-----");
    draft.public_key.replace_text("ssh-ed25519 AAAA...");
    draft.fingerprint.replace_text("SHA256:example-fingerprint");
    draft.name.replace_text("Laptop key");
    state.ssh_tools = Some(draft);
}

pub fn scene_ssh_approval(state: &mut State) {
    scene_populated(state);
    state.agent_running = true;
    state.agent_socket = "/run/user/1000/kurama-ssh-agent.sock".to_owned();
    state.approvals = vec![Approval {
        id: 42,
        title: "Sign a Git commit?".to_owned(),
        action: "git commit -S".to_owned(),
        key_name: "Laptop key".to_owned(),
        client: "git (ssh)".to_owned(),
    }];
    state.notice = Some("SSH approval requested".to_owned());
}

pub fn scene_create_card(state: &mut State) {
    scene_populated(state);
    let mut draft = start_create();
    draft.item_type = 3;
    state.create = Some(draft);
}

pub fn scene_create_identity(state: &mut State) {
    scene_populated(state);
    let mut draft = start_create();
    draft.item_type = 4;
    state.create = Some(draft);
}

pub fn scene_create_ssh(state: &mut State) {
    scene_populated(state);
    let mut draft = start_create();
    draft.item_type = 5;
    state.create = Some(draft);
}

pub fn scene_create_fields(state: &mut State) {
    scene_populated(state);
    let mut draft = start_create();
    add_create_field_raw(&mut draft);
    state.create = Some(draft);
}

pub fn scene_create_uris(state: &mut State) {
    scene_populated(state);
    let mut draft = start_create();
    draft.uris[0] = TextEdit::new("https://github.com");
    draft.uris.push(TextEdit::new("https://docs.example.com"));
    state.create = Some(draft);
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
        folder_id: "folder-personal".to_owned(),
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
            folder_id: folder.to_lowercase(),
            updated: "2026-01-02T03:04:05Z".to_owned(),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_challenges_switch_the_active_form() {
        let mut state = State::default();
        state.auth = Some(empty_auth_draft("https://vault.example.com"));
        apply_auth_outcome(
            &mut state,
            AuthOutcome {
                ok: true,
                two_factor_required: true,
                two_factor_providers: vec![0],
                ..AuthOutcome::default()
            },
        );
        assert!(matches!(
            state.auth.as_ref().map(|auth| auth.mode.clone()),
            Some(AuthMode::TwoFactor {
                provider: Some(0),
                ..
            })
        ));

        apply_auth_outcome(
            &mut state,
            AuthOutcome {
                ok: true,
                new_device_verification_required: true,
                ..AuthOutcome::default()
            },
        );
        assert_eq!(
            state.auth.as_ref().map(|auth| auth.mode.clone()),
            Some(AuthMode::NewDevice)
        );
    }

    #[test]
    fn fido2_challenge_switches_to_security_key_form() {
        let mut state = State::default();
        state.auth = Some(empty_auth_draft("https://vault.example.com"));
        apply_auth_outcome(
            &mut state,
            AuthOutcome {
                ok: false,
                two_factor_provider: Some(7),
                fido2_status: Some("no_device".to_owned()),
                error: Some("Security key not found".to_owned()),
                ..AuthOutcome::default()
            },
        );
        assert!(matches!(
            state.auth.as_ref().map(|auth| auth.mode.clone()),
            Some(AuthMode::Fido2 {
                status: Some(status),
                ..
            }) if status == "no_device"
        ));
        assert_eq!(state.notice.as_deref(), Some("Security key not found"));
    }

    #[test]
    fn session_errors_return_to_the_account_form() {
        let mut state = State::default();
        scene_populated(&mut state);
        apply(
            &mut state,
            Event::Error("Session expired. Log in again.".to_owned()),
        );
        assert_eq!(state.snapshot.status, "unauthenticated");
        assert!(state.rows.is_empty());
        assert!(state.auth.is_some());
        assert_eq!(
            state.notice.as_deref(),
            Some("Your session expired. Sign in again.")
        );
    }

    #[test]
    fn auth_validation_keeps_the_draft_open() {
        let mut state = State::default();
        scene_account(&mut state);
        submit_auth(&mut state);
        assert_eq!(
            state.form_error.as_deref(),
            Some("Email and password are required.")
        );
        assert!(state.auth.is_some());
    }

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
    fn folder_cycling_sets_draft_folder_ids() {
        let mut state = State::default();
        apply(
            &mut state,
            Event::Folders(vec![
                crate::backend::FolderMeta {
                    id: "folder-a".to_owned(),
                    name: "Alpha".to_owned(),
                },
                crate::backend::FolderMeta {
                    id: "folder-b".to_owned(),
                    name: "Beta".to_owned(),
                },
            ]),
        );
        assert_eq!(folder_label(&state, Some("folder-b")), "Beta");

        state.create = Some(start_create());
        cycle_create_folder(&mut state, 1);
        assert_eq!(
            state.create.as_ref().unwrap().folder_id.as_deref(),
            Some("folder-a")
        );
        cycle_create_folder(&mut state, 1);
        assert_eq!(
            state.create.as_ref().unwrap().folder_id.as_deref(),
            Some("folder-b")
        );
        cycle_create_folder(&mut state, -1);
        assert_eq!(
            state.create.as_ref().unwrap().folder_id.as_deref(),
            Some("folder-a")
        );
        cycle_create_folder(&mut state, -1);
        assert_eq!(state.create.as_ref().unwrap().folder_id, None);
    }

    #[test]
    fn create_draft_submits_name_and_type() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        state.create = Some(start_create());
        set_create_type(&mut state, 2);
        toggle_create_favorite(&mut state);
        state
            .create
            .as_mut()
            .unwrap()
            .name
            .replace_text("Recovery sheet");
        submit_create(&mut state);
        assert!(state.create.is_none());
        assert!(rx.try_iter().any(|request| matches!(
            request,
            Request::CreateItem { name, item_type, favorite, .. }
                if name == "Recovery sheet" && item_type == 2 && favorite
        )));

        apply(
            &mut state,
            Event::ItemCreated {
                id: "new-1".to_owned(),
                name: "Recovery sheet".to_owned(),
            },
        );
        assert_eq!(state.selected.as_deref(), Some("new-1"));
        assert_eq!(state.notice.as_deref(), Some("Created Recovery sheet"));
        assert!(rx
            .try_iter()
            .any(|request| matches!(request, Request::Refresh)));
    }

    #[test]
    fn empty_create_draft_is_kept_for_retry() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        state.create = Some(start_create());
        submit_create(&mut state);
        assert!(rx.try_iter().next().is_none());
        assert!(state.create.is_some());
        assert_eq!(state.form_error.as_deref(), Some("Item name is required."));
    }

    #[test]
    fn edit_draft_submits_metadata_and_prefilled_username() {
        let mut state = State::default();
        scene_populated(&mut state);
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);

        start_edit(&mut state);
        assert_eq!(state.edit.as_ref().unwrap().id, "1");
        toggle_edit_favorite(&mut state);
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
            Request::UpdateItem { id, name, favorite, username, password, uris, .. }
                if id == "1"
                    && name == "Renamed GitHub"
                    && !favorite
                    && username.as_deref() == Some("dev@example.com")
                    && password.is_none()
                    && uris == vec!["https://github.com", "https://docs.example.com"]
        )));

        apply(
            &mut state,
            Event::ItemRenamed {
                id: "1".to_owned(),
                name: "Renamed GitHub".to_owned(),
                favorite: false,
            },
        );
        assert_eq!(state.rows[0].name, "Renamed GitHub");
        assert!(!state.rows[0].favorite);
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
        assert_eq!(state.form_error.as_deref(), Some("Item name is required."));
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
                preview: true,
                result: crate::backend::AttachmentResult {
                    filename: "notes.txt".to_owned(),
                    path: "/tmp/notes.txt".to_owned(),
                    text: Some(Secret::new("shared notes".to_owned())),
                    is_image: false,
                    is_text: true,
                },
            },
        );
        assert_eq!(state.notice.as_deref(), Some("Previewing notes.txt"));
        assert_eq!(
            state
                .attachment_preview
                .as_ref()
                .map(|view| view.filename.as_str()),
            Some("notes.txt")
        );
        assert_eq!(
            backend::attachment_text("notes.txt").as_deref(),
            Some("shared notes")
        );
        apply(
            &mut state,
            Event::AttachmentOpened {
                preview: false,
                result: crate::backend::AttachmentResult {
                    filename: "receipt.pdf".to_owned(),
                    path: "/tmp/receipt.pdf".to_owned(),
                    text: None,
                    is_image: false,
                    is_text: false,
                },
            },
        );
        assert_eq!(state.notice.as_deref(), Some("Saved receipt.pdf"));
        assert_eq!(backend::attachment_text("notes.txt"), None);
        close_attachment(&mut state);
        assert!(state.attachment_preview.is_none());
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

    #[test]
    fn send_submission_queues_redacted_request() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        state.send_form = Some(SendDraft {
            name: TextEdit::new("Notes"),
            text: TextEdit::new("private-share-text").secure(),
            hidden: true,
            max_access: TextEdit::new("3"),
            expiration: TextEdit::new("2026-03-01"),
            password: TextEdit::new("send-passphrase").secure(),
        });

        submit_send(&mut state);

        assert!(state.send_form.is_some());
        let requests: Vec<Request> = rx.try_iter().collect();
        match &requests[0] {
            Request::CreateSend {
                name,
                text,
                hidden,
                max_access,
                expiration,
                password,
            } => {
                assert_eq!(name, "Notes");
                assert_eq!(text.as_str(), "private-share-text");
                assert!(*hidden);
                assert_eq!(*max_access, Some(3));
                assert_eq!(expiration.as_deref(), Some("2026-03-01"));
                assert_eq!(
                    password.as_ref().map(Secret::as_str),
                    Some("send-passphrase")
                );
                let debug = format!("{requests:?}");
                assert!(!debug.contains("private-share-text"));
                assert!(!debug.contains("send-passphrase"));
            }
            other => panic!("unexpected request: {other:?}"),
        }
    }

    #[test]
    fn send_events_expose_links_only_through_the_store() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        let row = SendRow {
            id: "send-1".to_owned(),
            name: "Shared".to_owned(),
            disabled: false,
            expiration: String::new(),
            max_access: 0,
            access_count: 0,
            url: Secret::new("https://example.test/send#capability".to_owned()),
        };
        let debug = format!("{row:?}");
        assert!(!debug.contains("capability"));

        apply(&mut state, Event::Sends(vec![row]));

        assert_eq!(state.sends.len(), 1);
        assert_eq!(state.send_selected.as_deref(), Some("send-1"));
        assert_eq!(
            backend::send_url("send-1").as_deref(),
            Some("https://example.test/send#capability")
        );
        let requests: Vec<Request> = rx.try_iter().collect();
        assert!(requests.is_empty());
    }

    #[test]
    fn send_delete_confirmation_queues_delete() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        request_delete_send(&mut state, "send-2".to_owned());
        assert_eq!(state.send_pending_delete.as_deref(), Some("send-2"));

        confirm_delete_send(&mut state);

        assert!(state.send_pending_delete.is_none());
        assert!(matches!(
            rx.try_iter().next(),
            Some(Request::DeleteSend { id }) if id == "send-2"
        ));
    }

    #[test]
    fn ssh_tool_generation_uses_selected_algorithm() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        open_ssh_tools(&mut state);
        let _: Vec<Request> = rx.try_iter().collect();
        cycle_ssh_algorithm(&mut state, 1);
        state
            .ssh_tools
            .as_mut()
            .unwrap()
            .comment
            .replace_text("laptop");

        submit_ssh_generate(&mut state);

        assert!(matches!(
            rx.try_iter().next(),
            Some(Request::GenerateSshKey { algorithm, comment })
                if algorithm == "rsa-2048" && comment == "laptop"
        ));
    }

    #[test]
    fn ssh_tool_import_validates_and_redacts_private_key() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        open_ssh_tools(&mut state);
        let _: Vec<Request> = rx.try_iter().collect();
        submit_ssh_import(&mut state);
        assert!(state.form_error.is_some());
        assert!(rx.try_iter().next().is_none());

        state
            .ssh_tools
            .as_mut()
            .unwrap()
            .private_key
            .replace_text("private-key-material");
        submit_ssh_import(&mut state);
        let requests: Vec<Request> = rx.try_iter().collect();
        assert!(state.form_error.is_none());
        assert!(!format!("{requests:?}").contains("private-key-material"));
        assert!(matches!(requests[0], Request::ImportSshKey { .. }));
    }

    #[test]
    fn ssh_tool_results_fill_the_draft_for_saving() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        open_ssh_tools(&mut state);
        let _: Vec<Request> = rx.try_iter().collect();
        apply(
            &mut state,
            Event::SshKeyGenerated {
                private: Secret::new("private-key-material".to_owned()),
                public: "ssh-ed25519 AAAA".to_owned(),
                fingerprint: "SHA256:fp".to_owned(),
                algorithm: "ED25519".to_owned(),
            },
        );
        let draft = state.ssh_tools.as_ref().unwrap();
        assert_eq!(draft.public_key.text, "ssh-ed25519 AAAA");
        assert_eq!(draft.fingerprint.text, "SHA256:fp");
        assert_eq!(draft.algorithm_label, "ED25519");

        save_ssh_tool_key(&mut state);
        let requests: Vec<Request> = rx.try_iter().collect();
        assert!(!format!("{requests:?}").contains("private-key-material"));
        assert!(matches!(requests[0], Request::CreateSshKey { .. }));
    }

    #[test]
    fn ssh_agent_events_update_status_and_clear_answered_prompts() {
        let mut state = State::default();
        let (tx, rx) = mpsc::channel();
        state.requests = Some(tx);
        apply(
            &mut state,
            Event::AgentStatus {
                running: true,
                socket: "/run/kurama-agent.sock".to_owned(),
            },
        );
        assert!(state.agent_running);
        assert_eq!(state.agent_socket, "/run/kurama-agent.sock");

        state.approvals = vec![Approval {
            id: 7,
            title: "Sign a commit?".to_owned(),
            action: "git commit".to_owned(),
            key_name: "Laptop".to_owned(),
            client: "git".to_owned(),
        }];
        answer_approval(&mut state, 7, true);
        assert!(state.approvals.is_empty());
        assert!(matches!(
            rx.try_iter().next(),
            Some(Request::AgentReply {
                id: 7,
                approve: true
            })
        ));

        apply(
            &mut state,
            Event::ApprovalAnswered {
                id: 7,
                approve: true,
            },
        );
        assert_eq!(state.notice.as_deref(), Some("SSH request approved"));
    }
}
