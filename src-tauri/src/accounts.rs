use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::icons;
use crate::theme::Theme;

pub const COLORS: &[&str] = &[
    "#e8a54b", "#6ea8fe", "#7dcea0", "#e07a5f", "#c084fc", "#5ec8d8", "#f0c14b", "#f472b6",
    "#a3e635", "#fb7185",
];

pub const SUITE: &[(&str, &str, &str)] = &[
    ("mail", "Mail", "https://mail.google.com/"),
    (
        "calendar",
        "Calendar",
        "https://calendar.google.com/calendar/r",
    ),
    ("drive", "Drive", "https://drive.google.com/drive/my-drive"),
    ("docs", "Docs", "https://docs.google.com/document/u/0/"),
    (
        "sheets",
        "Sheets",
        "https://docs.google.com/spreadsheets/u/0/",
    ),
    (
        "slides",
        "Slides",
        "https://docs.google.com/presentation/u/0/",
    ),
    ("meet", "Meet", "https://meet.google.com/"),
    ("chat", "Chat", "https://chat.google.com/"),
    ("contacts", "Contacts", "https://contacts.google.com/"),
    ("keep", "Keep", "https://keep.google.com/"),
    ("tasks", "Tasks", "https://tasks.google.com/tasks/"),
    ("forms", "Forms", "https://docs.google.com/forms/"),
    ("sites", "Sites", "https://sites.google.com/"),
    ("groups", "Groups", "https://groups.google.com/my-groups"),
    ("admin", "Admin", "https://admin.google.com/"),
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BrowserIdentity {
    Chrome,
    System,
}

impl Default for BrowserIdentity {
    fn default() -> Self {
        Self::Chrome
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub label: String,
    pub color: String,
    /// The Google address signed in to this account, read from Gmail's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Documents and other pages open as tabs under this account.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    /// `t` and hex digits, so view labels still split on their last `-`.
    pub id: String,
    pub url: String,
    pub title: String,
    /// The suite app it belongs to, for its icon: docs, sheets, slides, forms, drive or meet.
    pub kind: String,
}

/// Recognises a page that should open as a tab, returning its kind and a key
/// that stays the same for every URL of the same document (edit, view, #gid).
pub fn document(raw: &str) -> Option<(&'static str, String)> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?;
    let segments = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    // Previews and embeds live in frames inside other pages and must stay there.
    if segments
        .iter()
        .any(|part| matches!(*part, "preview" | "embed" | "htmlview" | "pubhtml" | "pub"))
    {
        return None;
    }
    let doc = |kind: &'static str, at: usize| {
        // Paths look like /document/d/{id}/edit, with an optional /u/{n} before them.
        let rest = &segments[at..];
        let index = rest.iter().position(|part| *part == "d")?;
        let id = rest.get(index + 1).filter(|id| id.len() >= 10)?;
        Some((kind, format!("{kind}:{id}")))
    };
    match host {
        "docs.google.com" => {
            let at = if segments.first() == Some(&"u") { 2 } else { 0 };
            match *segments.get(at)? {
                "document" => doc("docs", at),
                "spreadsheets" => doc("sheets", at),
                "presentation" => doc("slides", at),
                "forms" => doc("forms", at),
                "drawings" => doc("drive", at),
                _ => None,
            }
        }
        "meet.google.com" => {
            // Meetings run in the app only where the webview has WebRTC.
            // Elsewhere `calls` takes them before they get this far.
            let code = segments.first()?;
            crate::calls::is_meeting(raw).then(|| ("meet", format!("meet:{code}")))
        }
        "drive.google.com" => {
            let at = if segments.first() == Some(&"u") { 2 } else { 0 };
            if segments.get(at) == Some(&"file") {
                doc("drive", at)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// A home list's "new" button, which should become a tab rather than replace the list.
pub fn is_create(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|url| {
        url.host_str() == Some("docs.google.com")
            && url
                .path_segments()
                .is_some_and(|mut parts| parts.any(|part| part == "create"))
    })
}

/// Where the new-document buttons point.
pub fn new_document_url(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "docs" => "https://docs.google.com/document/create",
        "sheets" => "https://docs.google.com/spreadsheets/create",
        "slides" => "https://docs.google.com/presentation/create",
        "forms" => "https://docs.google.com/forms/create",
        _ => return None,
    })
}

pub fn is_tab_id(id: &str) -> bool {
    id.len() > 1 && id.starts_with('t') && id[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteItem {
    pub id: String,
    pub label: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub accounts: Vec<Account>,
    pub active_id: Option<String>,
    pub browser_identity: BrowserIdentity,
    pub rail_collapsed: bool,
    pub fronts: HashMap<String, String>,
    /// Unread inbox messages by account id, once Gmail has reported them.
    pub unread: HashMap<String, u32>,
    pub hidden_apps: Vec<String>,
    /// The browser that calls and Meet open in, if one is installed.
    pub call_browser: Option<String>,
    /// Whether Meet and calls run inside the app.
    pub calls_native: bool,
    /// Accounts whose views are closed while a feed view watches for mail.
    pub asleep: Vec<String>,
    pub sleep_after: Option<u32>,
    pub page_theme: PageTheme,
    pub suite: Vec<SuiteItem>,
    pub theme: Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreFile {
    #[serde(default)]
    accounts: Vec<Account>,
    #[serde(default)]
    active_id: Option<String>,
    #[serde(default)]
    browser_identity: BrowserIdentity,
    #[serde(default)]
    rail_collapsed: bool,
    #[serde(default)]
    hidden_apps: Vec<String>,
    #[serde(default)]
    fronts: HashMap<String, String>,
    /// Minutes before an account that is not in use goes to sleep. None keeps
    /// every account loaded.
    #[serde(default = "default_sleep_after")]
    sleep_after: Option<u32>,
    #[serde(default)]
    page_theme: PageTheme,
}

/// Whether Google pages take the Omarchy palette, and their documents with them.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageTheme {
    pub pages: bool,
    pub documents: bool,
}

impl Default for PageTheme {
    fn default() -> Self {
        Self {
            pages: true,
            documents: true,
        }
    }
}

fn default_sleep_after() -> Option<u32> {
    Some(5)
}

#[derive(Debug)]
pub struct DeskState {
    path: PathBuf,
    sessions_dir: PathBuf,
    accounts: Vec<Account>,
    active_id: Option<String>,
    browser_identity: BrowserIdentity,
    rail_collapsed: bool,
    /// The suite app each account was last showing. Absent means mail.
    fronts: HashMap<String, String>,
    unread: HashMap<String, u32>,
    hidden_apps: Vec<String>,
    sleep_after: Option<u32>,
    page_theme: PageTheme,
    theme: Theme,
}

impl DeskState {
    pub fn load(app_data: &Path) -> Result<Self> {
        fs::create_dir_all(app_data).context("create app data dir")?;
        let path = app_data.join("accounts.json");
        let sessions_dir = app_data.join("sessions");
        fs::create_dir_all(&sessions_dir).context("create session dir")?;
        let file = if path.exists() {
            let raw = fs::read_to_string(&path).context("read accounts")?;
            serde_json::from_str(&raw).context("parse accounts")?
        } else {
            StoreFile {
                accounts: Vec::new(),
                active_id: None,
                browser_identity: BrowserIdentity::Chrome,
                rail_collapsed: false,
                hidden_apps: Vec::new(),
                fronts: HashMap::new(),
                sleep_after: default_sleep_after(),
                page_theme: PageTheme::default(),
            }
        };
        let mut state = Self {
            path,
            sessions_dir,
            accounts: file.accounts,
            active_id: file.active_id,
            browser_identity: file.browser_identity,
            rail_collapsed: file.rail_collapsed,
            fronts: file.fronts,
            unread: HashMap::new(),
            hidden_apps: file.hidden_apps,
            sleep_after: file.sleep_after,
            page_theme: file.page_theme,
            theme: Theme::load(),
        };
        state
            .accounts
            .retain(|account| is_safe_id(&account.id) && is_hex_color(&account.color));
        for account in &mut state.accounts {
            account
                .tabs
                .retain(|tab| is_tab_id(&tab.id) && crate::policy::is_in_app(&tab.url));
        }
        let known = |id: &str, front: &str| {
            state.accounts.iter().any(|account| {
                account.id == id
                    && (suite_url(front).is_some()
                        || account.tabs.iter().any(|tab| tab.id == front))
            })
        };
        let fronts = std::mem::take(&mut state.fronts);
        state.fronts = fronts
            .into_iter()
            .filter(|(id, front)| known(id, front))
            .collect();
        if state
            .active_id
            .as_ref()
            .is_some_and(|id| !state.accounts.iter().any(|account| &account.id == id))
        {
            state.active_id = state.accounts.first().map(|account| account.id.clone());
        }
        Ok(state)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            accounts: self.accounts.clone(),
            active_id: self.active_id.clone(),
            browser_identity: self.browser_identity,
            rail_collapsed: self.rail_collapsed,
            fronts: self.fronts.clone(),
            unread: self.unread.clone(),
            hidden_apps: self.hidden_apps.clone(),
            call_browser: crate::calls::browser().map(|(_, name)| name.to_owned()),
            calls_native: crate::calls::native(),
            asleep: crate::lifecycle::asleep(),
            sleep_after: self.sleep_after,
            page_theme: self.page_theme,
            suite: SUITE
                .iter()
                .map(|(id, label, _)| SuiteItem {
                    id: (*id).to_owned(),
                    label: (*label).to_owned(),
                    icon: icons::service_icon(id),
                })
                .collect(),
            theme: self.theme.clone(),
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    pub fn browser_identity(&self) -> BrowserIdentity {
        self.browser_identity
    }

    pub fn rail_collapsed(&self) -> bool {
        self.rail_collapsed
    }

    /// Browser profiles for calls, one per account.
    pub fn call_profiles(&self) -> PathBuf {
        self.sessions_dir.with_file_name("calls")
    }

    pub fn session_dir(&self, id: &str) -> PathBuf {
        self.sessions_dir.join(id)
    }

    pub fn set_active(&mut self, id: Option<String>) {
        self.active_id = id;
    }

    pub fn front(&self, id: &str) -> &str {
        self.fronts.get(id).map_or("mail", String::as_str)
    }

    pub fn set_front(&mut self, id: &str, suite: &str) {
        self.fronts.insert(id.to_owned(), suite.to_owned());
    }

    pub fn tab(&self, account_id: &str, tab_id: &str) -> Option<&Tab> {
        self.accounts
            .iter()
            .find(|account| account.id == account_id)?
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
    }

    /// The tab already showing this document, if any.
    pub fn tab_for(&self, account_id: &str, key: &str) -> Option<&Tab> {
        self.accounts
            .iter()
            .find(|account| account.id == account_id)?
            .tabs
            .iter()
            .find(|tab| document(&tab.url).is_some_and(|(_, other)| other == key))
    }

    pub fn add_tab(&mut self, account_id: &str, url: &str, kind: &str) -> Result<Tab> {
        let account = self.account_mut(account_id).context("account not found")?;
        let id = format!("t{}", &Uuid::new_v4().simple().to_string()[..8]);
        let tab = Tab {
            id,
            url: url.to_owned(),
            title: String::new(),
            kind: kind.to_owned(),
        };
        account.tabs.push(tab.clone());
        self.save()?;
        Ok(tab)
    }

    /// Returns whether anything changed. The title is cleaned of Google's app suffix.
    pub fn update_tab(
        &mut self,
        account_id: &str,
        tab_id: &str,
        title: Option<&str>,
        url: Option<&str>,
    ) -> Result<bool> {
        let Some(tab) = self
            .account_mut(account_id)
            .and_then(|account| account.tabs.iter_mut().find(|tab| tab.id == tab_id))
        else {
            return Ok(false);
        };
        let mut changed = false;
        if let Some(title) = title
            .map(clean_title)
            .filter(|title| !title.is_empty() && *title != tab.title)
        {
            tab.title = title;
            changed = true;
        }
        // Only documents are kept, so a sign-in page never replaces the saved address.
        if let Some(url) = url.filter(|url| *url != tab.url && document(url).is_some()) {
            tab.url = url.to_owned();
            if let Some((kind, _)) = document(url) {
                tab.kind = kind.to_owned();
            }
            changed = true;
        }
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn remove_tab(&mut self, account_id: &str, tab_id: &str) -> Result<Option<Tab>> {
        let Some(account) = self.account_mut(account_id) else {
            return Ok(None);
        };
        let Some(index) = account.tabs.iter().position(|tab| tab.id == tab_id) else {
            return Ok(None);
        };
        let tab = account.tabs.remove(index);
        if self.fronts.get(account_id).map(String::as_str) == Some(tab_id) {
            self.fronts.remove(account_id);
        }
        self.save()?;
        Ok(Some(tab))
    }

    pub fn move_tab(&mut self, account_id: &str, tab_id: &str, delta: i32) -> Result<()> {
        let account = self.account_mut(account_id).context("account not found")?;
        let index = account
            .tabs
            .iter()
            .position(|tab| tab.id == tab_id)
            .context("tab not found")?;
        let next = index as i32 + delta;
        if next < 0 || next as usize >= account.tabs.len() {
            return Ok(());
        }
        account.tabs.swap(index, next as usize);
        self.save()
    }

    /// Returns whether the count changed.
    pub fn set_unread(&mut self, id: &str, count: u32) -> bool {
        self.unread.insert(id.to_owned(), count) != Some(count)
    }

    /// Returns whether the address changed. Saved, so it shows before Gmail loads.
    pub fn set_email(&mut self, id: &str, email: &str) -> Result<bool> {
        let Some(account) = self.account_mut(id) else {
            return Ok(false);
        };
        if account.email.as_deref() == Some(email) {
            return Ok(false);
        }
        account.email = Some(email.to_owned());
        self.save()?;
        Ok(true)
    }

    pub fn sleep_after(&self) -> Option<std::time::Duration> {
        self.sleep_after
            .map(|minutes| std::time::Duration::from_secs(u64::from(minutes) * 60))
    }

    pub fn set_sleep_after(&mut self, minutes: Option<u32>) -> Result<()> {
        if minutes.is_some_and(|minutes| minutes == 0 || minutes > 24 * 60) {
            anyhow::bail!("sleep time must be between a minute and a day");
        }
        self.sleep_after = minutes;
        self.save()
    }

    pub fn set_page_theme(&mut self, page_theme: PageTheme) -> Result<()> {
        self.page_theme = page_theme;
        self.save()
    }

    /// The styles every Google page should carry right now.
    pub fn page_styles(&self) -> crate::pages::Styles {
        crate::pages::styles(
            &self.theme,
            self.page_theme.pages,
            self.page_theme.documents,
        )
    }

    pub fn set_app_hidden(&mut self, suite: &str, hidden: bool) -> Result<()> {
        if suite_url(suite).is_none() || suite == "mail" {
            anyhow::bail!("that app cannot be hidden");
        }
        self.hidden_apps.retain(|id| id != suite);
        if hidden {
            self.hidden_apps.push(suite.to_owned());
        }
        self.save()
    }

    pub fn set_rail_collapsed(&mut self, collapsed: bool) {
        self.rail_collapsed = collapsed;
    }

    pub fn set_browser_identity(&mut self, identity: BrowserIdentity) {
        self.browser_identity = identity;
    }

    pub fn create(&mut self, label: &str) -> Result<Account> {
        let label = clean_label(label)?;
        let id = Uuid::new_v4().simple().to_string();
        let color = COLORS[self.accounts.len() % COLORS.len()].to_owned();
        let account = Account {
            id,
            label,
            color,
            email: None,
            tabs: Vec::new(),
        };
        self.accounts.push(account.clone());
        self.active_id = Some(account.id.clone());
        self.save()?;
        Ok(account)
    }

    pub fn rename(&mut self, id: &str, label: &str) -> Result<()> {
        let label = clean_label(label)?;
        let account = self.account_mut(id).context("account not found")?;
        account.label = label;
        self.save()
    }

    pub fn set_color(&mut self, id: &str, color: &str) -> Result<()> {
        if !is_hex_color(color) {
            anyhow::bail!("color must be a #rrggbb hex value");
        }
        let account = self.account_mut(id).context("account not found")?;
        account.color = color.to_ascii_lowercase();
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> Result<Account> {
        let index = self
            .accounts
            .iter()
            .position(|account| account.id == id)
            .context("account not found")?;
        let account = self.accounts.remove(index);
        self.fronts.remove(id);
        self.unread.remove(id);
        if self.active_id.as_deref() == Some(id) {
            self.active_id = self
                .accounts
                .get(index.min(self.accounts.len().saturating_sub(1)))
                .map(|account| account.id.clone());
        }
        self.save()?;
        Ok(account)
    }

    pub fn move_by(&mut self, id: &str, delta: i32) -> Result<()> {
        let index = self
            .accounts
            .iter()
            .position(|account| account.id == id)
            .context("account not found")?;
        let next = index as i32 + delta;
        if next < 0 || next as usize >= self.accounts.len() {
            return Ok(());
        }
        self.accounts.swap(index, next as usize);
        self.save()
    }

    pub fn cycle(&mut self, delta: i32) -> Option<String> {
        if self.accounts.is_empty() || delta == 0 {
            return self.active_id.clone();
        }
        let len = self.accounts.len() as i32;
        let current = self
            .active_id
            .as_ref()
            .and_then(|id| self.accounts.iter().position(|account| &account.id == id))
            .unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(len) as usize;
        let id = self.accounts[next].id.clone();
        self.active_id = Some(id.clone());
        Some(id)
    }

    pub fn index_id(&self, index: usize) -> Option<String> {
        self.accounts.get(index).map(|account| account.id.clone())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).context("create account store dir")?;
        }
        let file = StoreFile {
            accounts: self.accounts.clone(),
            active_id: self.active_id.clone(),
            browser_identity: self.browser_identity,
            rail_collapsed: self.rail_collapsed,
            hidden_apps: self.hidden_apps.clone(),
            fronts: self.fronts.clone(),
            sleep_after: self.sleep_after,
            page_theme: self.page_theme,
        };
        let data = serde_json::to_vec_pretty(&file).context("serialize accounts")?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, data).context("write accounts")?;
        fs::rename(&tmp, &self.path).context("replace accounts")?;
        Ok(())
    }

    fn account_mut(&mut self, id: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|account| account.id == id)
    }
}

/// "Budget 2026 - Google Sheets" reads as "Budget 2026".
fn clean_title(title: &str) -> String {
    let title = title.trim();
    for suffix in [
        " - Google Docs",
        " - Google Sheets",
        " - Google Slides",
        " - Google Forms",
        " - Google Drive",
        " - Google Drawings",
        " - Google Meet",
    ] {
        if let Some(stripped) = title.strip_suffix(suffix) {
            return stripped.trim().to_owned();
        }
    }
    title
        .strip_prefix("Meet - ")
        .unwrap_or(title)
        .trim()
        .to_owned()
}

pub fn suite_url(id: &str) -> Option<&'static str> {
    SUITE
        .iter()
        .find(|(suite_id, _, _)| *suite_id == id)
        .map(|(_, _, url)| *url)
}

pub fn is_safe_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn is_hex_color(color: &str) -> bool {
    let Some(hex) = color.strip_prefix('#') else {
        return false;
    };
    hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn clean_label(label: &str) -> Result<String> {
    let label = label.trim();
    if label.is_empty() {
        anyhow::bail!("label is empty");
    }
    if label.chars().count() > 48 {
        anyhow::bail!("label is too long");
    }
    if label.chars().any(|ch| ch.is_control()) {
        anyhow::bail!("label has control characters");
    }
    Ok(label.to_owned())
}

pub fn store_identifier(id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bytes = id.as_bytes();
    for (index, byte) in bytes.iter().take(32).enumerate() {
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => 0,
        };
        out[index / 2] = (out[index / 2] << 4) | nibble;
    }
    out
}

pub fn user_agent(identity: BrowserIdentity) -> Option<&'static str> {
    match identity {
        BrowserIdentity::System => None,
        BrowserIdentity::Chrome => Some(match () {
            #[cfg(target_os = "macos")]
            () => {
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
            }
            #[cfg(target_os = "windows")]
            () => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            () => {
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
            }
        }),
    }
}
