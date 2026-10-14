use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

use crate::accounts::{self, BrowserIdentity, DeskState};
use crate::calls;
use crate::policy;

pub struct AppState {
    pub desks: Mutex<DeskState>,
}

static POPUP_SEQ: AtomicU64 = AtomicU64::new(1);
static LAST_ACTION: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub fn lock(state: &AppState) -> std::sync::MutexGuard<'_, DeskState> {
    state.desks.lock().unwrap_or_else(|err| err.into_inner())
}

fn accept_action(key: &str) -> bool {
    let mut last = LAST_ACTION.lock().unwrap_or_else(|err| err.into_inner());
    if let Some((prev, at)) = last.as_ref()
        && prev == key
        && at.elapsed() < Duration::from_millis(200)
    {
        return false;
    }
    *last = Some((key.to_owned(), Instant::now()));
    true
}

pub fn restore(app: &AppHandle) -> Result<()> {
    let labels = app.webview_windows().keys().cloned().collect::<Vec<_>>();
    log::info!("windows at restore: {labels:?}");
    let state = app.state::<AppState>();
    let (active, others) = {
        let desks = lock(&state);
        let active = desks
            .active_id()
            .map(str::to_owned)
            .or_else(|| desks.accounts().first().map(|account| account.id.clone()));
        let others = desks
            .accounts()
            .iter()
            .map(|account| account.id.clone())
            .filter(|id| Some(id) != active.as_ref())
            .collect::<Vec<_>>();
        (active, others)
    };
    if let Some(id) = active {
        focus_account(app, &id, true)?;
    }
    // Accounts not in front start asleep, unless the user keeps them loaded.
    let keep = lock(&state).sleep_after().is_none();
    let warm = app.clone();
    std::thread::spawn(move || {
        if keep {
            warm_accounts(&warm, &others);
        } else {
            for id in &others {
                if let Err(err) = crate::lifecycle::sleep(&warm, id) {
                    log::warn!("account was not put to sleep: {err:#}");
                }
            }
        }
    });
    install_shell_close(app)
}

pub fn focus_account(app: &AppHandle, id: &str, force: bool) -> Result<()> {
    let suite = {
        let state = app.state::<AppState>();
        let desks = lock(&state);
        if !desks.accounts().iter().any(|account| account.id == id) {
            anyhow::bail!("account not found");
        }
        // Switching accounts must never launch a call window.
        let front = desks.front(id);
        let call = front == "meet" || desks.tab(id, front).is_some_and(|tab| tab.kind == "meet");
        if call && calls::external() {
            "mail"
        } else {
            front
        }
        .to_owned()
    };
    if !force && !accept_action(&format!("focus:{id}")) {
        return Ok(());
    }
    focus_surface(app, id, &suite)
}

pub fn warm_accounts(app: &AppHandle, ids: &[String]) {
    for id in ids {
        if let Err(err) = open_under(app, id, "mail") {
            log::warn!("account was not warmed: {err:#}");
        }
    }
}

pub fn show_shell(app: &AppHandle) -> Result<()> {
    let shell = app.get_window("shell").context("shell window missing")?;
    shell.show().context("show shell")?;
    let _ = shell.unminimize();
    shell.set_focus().context("focus shell")?;
    Ok(())
}

pub fn set_identity(app: &AppHandle, identity: BrowserIdentity) -> Result<()> {
    let active = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        desks.set_browser_identity(identity);
        desks.save()?;
        desks.active_id().map(str::to_owned)
    };
    // Views keep the user agent they were built with, so every one is rebuilt.
    for (_, webview) in app.webviews() {
        if webview.label().starts_with("view-") {
            let _ = webview.close();
        }
    }
    // Feed views went with the rest. Sleeping accounts wake and doze again
    // so they get new ones with the new identity.
    for id in crate::lifecycle::asleep() {
        crate::lifecycle::wake(app, &id);
        crate::lifecycle::sleep(app, &id)?;
    }
    if let Some(id) = active {
        focus_account(app, &id, true)?;
    } else {
        deliver(app)?;
    }
    Ok(())
}

pub fn remove_account(app: &AppHandle, id: &str) -> Result<()> {
    crate::lifecycle::wake(app, id);
    let next = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        if !accounts::is_safe_id(id) {
            anyhow::bail!("invalid account id");
        }
        let removed = desks.remove(id)?;
        let prefix = format!("view-{}-", removed.id);
        for (_, webview) in app.webviews() {
            if webview.label().starts_with(&prefix) {
                let _ = webview.close();
            }
        }
        for dir in [
            desks.session_dir(&removed.id),
            desks.call_profiles().join(&removed.id),
        ] {
            if dir.exists()
                && let Err(err) = std::fs::remove_dir_all(&dir)
            {
                log::warn!("session data was not removed: {err}");
            }
        }
        desks.active_id().map(str::to_owned)
    };
    if let Some(active) = next {
        focus_account(app, &active, true)?;
    } else {
        // Nothing left to show beside the rail, so it takes the whole window.
        crate::viewport::show_settings(app)?;
        deliver(app)?;
        show_shell(app)?;
    }
    Ok(())
}

pub fn deliver(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let (snapshot, title) = {
        let desks = lock(&state);
        let title = desks
            .active_id()
            .and_then(|id| desks.accounts().iter().find(|account| account.id == id))
            .map_or_else(
                || "Workspaced".to_owned(),
                |account| format!("Workspaced — {}", account.label),
            );
        let snapshot = desks.snapshot();
        // Bars and task switchers read the title, so it carries the unread total like a browser tab.
        let unread = snapshot.unread.values().sum::<u32>();
        let title = if unread > 0 {
            format!("({unread}) {title}")
        } else {
            title
        };
        (snapshot, title)
    };
    if let Some(shell) = app.get_window("shell") {
        let _ = shell.set_title(&title);
    }
    let _ = app.emit("accounts-updated", &snapshot);
    Ok(())
}

pub fn allow_navigation(app: &AppHandle, label: &str, raw: &str) -> bool {
    on_navigate(app, label, raw)
}

pub fn surface_label(account_id: &str, suite: &str) -> String {
    format!("view-{account_id}-{suite}")
}

fn account_id_from_label(label: &str) -> Option<String> {
    split_label(label).map(|(id, _)| id.to_owned())
}

/// Splits `view-{account}-{suite}` into its account id and suite id.
pub fn split_label(label: &str) -> Option<(&str, &str)> {
    label.strip_prefix("view-")?.rsplit_once('-')
}

/// Opens a Meet link in the account's browser profile. See `calls`.
pub fn open_call(app: &AppHandle, account_id: &str, url: &str) -> Result<()> {
    // A backstop against anything that asks over and over: one window per account at a time.
    static LAST: Mutex<Option<(String, Instant)>> = Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap_or_else(|err| err.into_inner());
        if last
            .as_ref()
            .is_some_and(|(id, at)| id == account_id && at.elapsed() < Duration::from_secs(4))
        {
            log::warn!("call launch ignored: another one started moments ago");
            return Ok(());
        }
        *last = Some((account_id.to_owned(), Instant::now()));
    }
    let profiles = {
        let state = app.state::<AppState>();
        let desks = lock(&state);
        if !desks
            .accounts()
            .iter()
            .any(|account| account.id == account_id)
        {
            anyhow::bail!("account not found");
        }
        desks.call_profiles()
    };
    calls::open(&profiles, account_id, url)
}

/// Sends a Meet link to the browser if there is one. Returns whether it did.
fn hand_off_call(app: &AppHandle, label: &str, url: &str) -> bool {
    if !calls::is_meeting(url) || !calls::external() {
        return false;
    }
    let Some(account_id) = account_id_from_label(label) else {
        return false;
    };
    if let Err(err) = open_call(app, &account_id, url) {
        log::warn!("call did not open: {err:#}");
    }
    true
}

/// Suite apps whose own lists of documents hand off to tabs when one is opened.
const HOME_APPS: &[&str] = &["docs", "sheets", "slides", "forms", "drive"];

/// Opens a page as a tab under an account, or switches to the tab already
/// showing that document.
pub fn open_tab(app: &AppHandle, account_id: &str, url: &str, kind: &str) -> Result<()> {
    if !policy::is_in_app(url) {
        anyhow::bail!("only Google pages open as tabs");
    }
    let tab_id = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        let existing = accounts::document(url)
            .and_then(|(_, key)| desks.tab_for(account_id, &key))
            .map(|tab| tab.id.clone());
        match existing {
            Some(id) => id,
            None => desks.add_tab(account_id, url, kind)?.id,
        }
    };
    focus_surface(app, account_id, &tab_id)
}

/// Opens a page as a tab without blocking the caller. Used from WebKit
/// callbacks, which run on the main thread that the switch waits on.
fn open_tab_later(app: &AppHandle, account_id: &str, url: &str, kind: &str) {
    let (app, account_id, url, kind) = (
        app.clone(),
        account_id.to_owned(),
        url.to_owned(),
        kind.to_owned(),
    );
    std::thread::spawn(move || {
        if let Err(err) = open_tab(&app, &account_id, &url, &kind) {
            log::warn!("tab did not open: {err:#}");
        }
    });
}

pub fn new_tab(app: &AppHandle, account_id: &str, kind: &str) -> Result<()> {
    let url = accounts::new_document_url(kind).context("unknown document type")?;
    let tab = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        desks.add_tab(account_id, url, kind)?
    };
    focus_surface(app, account_id, &tab.id)
}

pub fn close_tab(app: &AppHandle, account_id: &str, tab_id: &str) -> Result<()> {
    let fallback = {
        let state = app.state::<AppState>();
        let desks = lock(&state);
        if desks.front(account_id) != tab_id {
            None
        } else {
            // Like a browser: the neighbouring tab, or mail when it was the last.
            let tabs = &desks
                .accounts()
                .iter()
                .find(|account| account.id == account_id)
                .context("account not found")?
                .tabs;
            let index = tabs.iter().position(|tab| tab.id == tab_id);
            Some(
                index
                    .and_then(|index| {
                        tabs.get(index + 1)
                            .or_else(|| index.checked_sub(1).and_then(|i| tabs.get(i)))
                    })
                    .map_or_else(|| "mail".to_owned(), |tab| tab.id.clone()),
            )
        }
    };
    if let Some(next) = fallback {
        focus_surface(app, account_id, &next)?;
    }
    if let Some(webview) = app.get_webview(&surface_label(account_id, tab_id)) {
        let _ = webview.close();
    }
    {
        let state = app.state::<AppState>();
        lock(&state).remove_tab(account_id, tab_id)?;
    }
    let _ = app.emit("surface-closed", (account_id, tab_id));
    deliver(app)
}

pub fn move_tab(app: &AppHandle, account_id: &str, tab_id: &str, delta: i32) -> Result<()> {
    {
        let state = app.state::<AppState>();
        lock(&state).move_tab(account_id, tab_id, delta)?;
    }
    deliver(app)
}

pub fn on_tab_page(
    app: &AppHandle,
    account_id: &str,
    tab_id: &str,
    title: Option<&str>,
    url: Option<&str>,
) {
    let changed = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        desks
            .update_tab(account_id, tab_id, title, url)
            .unwrap_or(false)
    };
    if changed {
        let _ = deliver(app);
    }
}

/// Closes an app view to free its memory. Mail is the account's home and stays.
pub fn close_surface(app: &AppHandle, account_id: &str, suite: &str) -> Result<()> {
    if suite == "mail" {
        anyhow::bail!("mail stays open for each account");
    }
    let Some(webview) = app.get_webview(&surface_label(account_id, suite)) else {
        return Ok(());
    };
    let showing = {
        let state = app.state::<AppState>();
        let desks = lock(&state);
        desks.front(account_id) == suite
    };
    if showing {
        // Put mail in front first so the view never goes blank.
        focus_surface(app, account_id, "mail")?;
    }
    webview.close().context("close app view")?;
    let _ = app.emit("surface-closed", (account_id, suite));
    Ok(())
}

pub fn reload_surface(app: &AppHandle, account_id: &str, suite: &str) -> Result<()> {
    let webview = app
        .get_webview(&surface_label(account_id, suite))
        .context("that app is not open")?;
    webview.eval("location.reload()").context("reload app view")
}

/// Gmail titles read `Inbox (12) - ada@example.com - Gmail`. Returns the inbox
/// count (only while the inbox is showing, other folders count other things)
/// and the signed-in address.
pub fn parse_gmail_title(title: &str) -> (Option<u32>, Option<String>) {
    let parts = title.split(" - ").map(str::trim).collect::<Vec<_>>();
    // The last part is "Gmail", or "<Organisation> Mail" on Workspace accounts.
    if parts.len() < 3
        || !parts
            .last()
            .is_some_and(|last| last.to_lowercase().ends_with("mail"))
    {
        return (None, None);
    }
    let email = parts
        .iter()
        .find(|part| part.contains('@') && !part.contains(' '))
        .map(|part| (*part).to_owned());
    let folder = parts[0];
    let count = if let Some(rest) = folder.strip_prefix("Inbox") {
        let rest = rest.trim();
        if rest.is_empty() {
            Some(0)
        } else {
            rest.strip_prefix('(')
                .and_then(|rest| rest.strip_suffix(')'))
                .and_then(|digits| digits.replace([',', '.', '\u{a0}'], "").parse().ok())
        }
    } else {
        None
    };
    (count, email)
}

pub fn on_mail_title(app: &AppHandle, account_id: &str, title: &str) {
    let (count, email) = parse_gmail_title(title);
    let (changed, arrived) = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        let before = desks.snapshot().unread.get(account_id).copied();
        let mut changed = count.is_some_and(|count| desks.set_unread(account_id, count));
        // The first count after launch is not news, only a rise after it.
        let arrived = matches!((before, count), (Some(before), Some(count)) if count > before);
        if let Some(email) = email {
            changed |= desks.set_email(account_id, &email).unwrap_or(false);
        }
        (changed, arrived)
    };
    if changed {
        let _ = deliver(app);
    }
    if arrived {
        // Lets the compositor flag the window (Hyprland marks it urgent).
        if let Some(shell) = app.get_window("shell")
            && !shell.is_focused().unwrap_or(true)
        {
            let _ = shell.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Download {
    url: String,
    name: String,
    path: Option<String>,
    /// "started", "done" or "failed".
    status: &'static str,
}

/// Downloads land in the downloads folder under a unique name (wry picks it).
/// The shell is told so it can show progress and offer to reveal the file.
pub fn on_download(app: &AppHandle, event: tauri::webview::DownloadEvent<'_>) {
    use tauri::webview::DownloadEvent;
    let download = match event {
        DownloadEvent::Requested { url, destination } => Download {
            url: url.to_string(),
            name: file_name(destination),
            path: None,
            status: "started",
        },
        DownloadEvent::Finished { url, path, success } => Download {
            url: url.to_string(),
            name: path.as_deref().map(file_name).unwrap_or_default(),
            path: path
                .filter(|_| success)
                .map(|path| path.to_string_lossy().into_owned()),
            status: if success { "done" } else { "failed" },
        },
        _ => return,
    };
    let _ = app.emit("download", download);
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Opens the file manager on a finished download. Only paths inside the
/// downloads folder are accepted, since this is reachable from the shell page.
pub fn reveal_download(app: &AppHandle, path: &str) -> Result<()> {
    let dir = app.path().download_dir().context("downloads folder")?;
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let path = std::fs::canonicalize(path).context("that download is gone")?;
    if !path.starts_with(&dir) {
        anyhow::bail!("not a download");
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .context("show download")
}

/// Handles a launch, or a second launch passed on by the running instance:
/// brings the window forward and opens any `mailto:` link in Gmail compose.
pub fn on_launch(app: &AppHandle, args: &[String]) {
    let mailto = args
        .iter()
        .find(|arg| arg.to_ascii_lowercase().starts_with("mailto:"))
        .cloned();
    let app = app.clone();
    // Switching waits on the main thread, and launches can arrive on it.
    std::thread::spawn(move || {
        let result = (|| {
            show_shell(&app)?;
            let Some(mailto) = mailto else {
                return Ok(());
            };
            compose(&app, &mailto)
        })();
        if let Err(err) = result {
            log::warn!("launch request failed: {err:#}");
        }
    });
}

/// Opens Gmail's compose for a `mailto:` link in the active account. Gmail
/// reads the address, subject and body out of the link itself.
fn compose(app: &AppHandle, mailto: &str) -> Result<()> {
    let account_id = {
        let state = app.state::<AppState>();
        let desks = lock(&state);
        desks
            .active_id()
            .map(str::to_owned)
            .or_else(|| desks.accounts().first().map(|account| account.id.clone()))
            .context("add an account first")?
    };
    let mut url = url::Url::parse("https://mail.google.com/mail/").context("compose url")?;
    url.query_pairs_mut()
        .append_pair("extsrc", "mailto")
        .append_pair("url", mailto);
    focus_surface(app, &account_id, "mail")?;
    app.get_webview(&surface_label(&account_id, "mail"))
        .context("mail is not open")?
        .navigate(url)
        .context("open compose")
}

/// A desktop notification from a Google page was clicked.
#[cfg(target_os = "linux")]
pub fn on_notification_clicked(app: &AppHandle, label: &str) {
    let Some((account_id, suite)) = split_label(label) else {
        return;
    };
    let (account_id, suite) = (account_id.to_owned(), suite.to_owned());
    let app = app.clone();
    // Switching waits on the main thread, and this runs on it.
    std::thread::spawn(move || {
        if let Err(err) = focus_surface(&app, &account_id, &suite).and_then(|_| show_shell(&app)) {
            log::warn!("notification switch failed: {err:#}");
        }
    });
}

pub fn open_under(app: &AppHandle, account_id: &str, suite: &str) -> Result<()> {
    let label = surface_label(account_id, suite);
    if app.get_webview(&label).is_some() {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let (parsed, session, identity, script, store_id) = {
        let desks = lock(&state);
        if !desks
            .accounts()
            .iter()
            .any(|account| account.id == account_id)
        {
            anyhow::bail!("account not found");
        }
        // Tabs are built on first use, so restored tabs cost nothing until clicked.
        let url = if suite == crate::lifecycle::POLL_SUITE {
            crate::lifecycle::FEED.to_owned()
        } else if accounts::is_tab_id(suite) {
            desks
                .tab(account_id, suite)
                .context("tab not found")?
                .url
                .clone()
        } else {
            accounts::suite_url(suite)
                .context("unknown suite app")?
                .to_owned()
        };
        let parsed: url::Url = url.parse().context("page url")?;
        (
            parsed,
            desks.session_dir(account_id),
            desks.browser_identity(),
            rail_script(&desks),
            accounts::store_identifier(account_id),
        )
    };
    crate::viewport::create_surface(
        app,
        &label,
        parsed,
        crate::viewport::Session {
            dir: session,
            identity,
            script,
            store_id,
        },
        surface_label(account_id, "mail"),
    )?;
    Ok(())
}

pub fn focus_surface(app: &AppHandle, account_id: &str, suite: &str) -> Result<()> {
    if calls::external() {
        if suite == "meet" {
            return open_call(app, account_id, "https://meet.google.com/");
        }
        // A meeting tab kept from a run where calls worked in the app.
        let meeting = {
            let state = app.state::<AppState>();
            let desks = lock(&state);
            desks
                .tab(account_id, suite)
                .filter(|tab| tab.kind == "meet")
                .map(|tab| tab.url.clone())
        };
        if let Some(url) = meeting {
            return open_call(app, account_id, &url);
        }
    }
    crate::lifecycle::wake(app, account_id);
    let label = surface_label(account_id, suite);
    if app.get_webview(&label).is_none() {
        open_under(app, account_id, suite)?;
    }
    crate::viewport::show_surface(app, &label)?;
    let previous = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        let previous = desks.active_id().map(str::to_owned);
        desks.set_front(account_id, suite);
        desks.set_active(Some(account_id.to_owned()));
        desks.save()?;
        previous
    };
    if let Some(previous) = previous.filter(|previous| previous != account_id) {
        crate::lifecycle::left(&previous);
    }
    deliver(app)
}

fn attach_guards<M: Manager<tauri::Wry>>(
    builder: WebviewWindowBuilder<'_, tauri::Wry, M>,
    app: AppHandle,
    label: String,
    popup: String,
    session: std::path::PathBuf,
    identity: BrowserIdentity,
) -> WebviewWindowBuilder<'_, tauri::Wry, M> {
    let app_nav = app.clone();
    let label_nav = label.clone();
    let app_pop = app;
    let parent = label;
    let builder = builder.on_navigation(move |url| {
        // A blank popup sent on to a call goes to the browser and closes here.
        if hand_off_call(&app_nav, &label_nav, url.as_str()) {
            let app = app_nav.clone();
            let popup = popup.clone();
            let _ = app_nav.run_on_main_thread(move || {
                if let Some(window) = app.get_webview_window(&popup) {
                    let _ = window.close();
                }
            });
            return false;
        }
        // Pages often open a blank popup and then send it to a document.
        // That document belongs in a tab, and the empty popup goes away.
        if let (Some((kind, _)), Some(account_id)) = (
            accounts::document(url.as_str()),
            account_id_from_label(&label_nav),
        ) {
            open_tab_later(&app_nav, &account_id, url.as_str(), kind);
            let app = app_nav.clone();
            let popup = popup.clone();
            let _ = app_nav.run_on_main_thread(move || {
                if let Some(window) = app.get_webview_window(&popup) {
                    let _ = window.close();
                }
            });
            return false;
        }
        on_navigate(&app_nav, &label_nav, url.as_str())
    });
    builder.on_new_window(move |url, features| {
        on_popup(&app_pop, &parent, session.clone(), url, features, identity)
    })
}

fn on_navigate(app: &AppHandle, label: &str, raw: &str) -> bool {
    if policy::is_sentinel(raw) {
        let raw = raw.to_owned();
        let app = app.clone();
        let label = label.to_owned();
        let _ = app.clone().run_on_main_thread(move || {
            if let Err(err) = dispatch_sentinel(&app, &label, &raw) {
                log::warn!("rail action failed: {err}");
            }
        });
        return false;
    }
    // Meet links are not handed off here. This hook also sees navigations in
    // frames, and Gmail and Calendar keep hidden Meet frames that reload on
    // their own, so every one of them would launch a browser window.
    if let Some((account_id, suite)) =
        split_label(label).filter(|(_, suite)| HOME_APPS.contains(suite))
        && accounts::is_create(raw)
    {
        open_tab_later(app, account_id, raw, suite);
        return false;
    }
    if let Some((kind, _)) = accounts::document(raw)
        && let Some((account_id, suite)) = split_label(label)
        && HOME_APPS.contains(&suite)
    {
        open_tab_later(app, account_id, raw, kind);
        return false;
    }
    if policy::is_in_app(raw) {
        return true;
    }
    if policy::is_external_openable(raw) {
        let raw = raw.to_owned();
        let app = app.clone();
        let _ = app
            .clone()
            .run_on_main_thread(move || open_external(&app, &raw));
    }
    false
}

pub(crate) fn on_popup(
    app: &AppHandle,
    parent_label: &str,
    session: std::path::PathBuf,
    url: url::Url,
    features: tauri::webview::NewWindowFeatures,
    identity: BrowserIdentity,
) -> NewWindowResponse<tauri::Wry> {
    if !policy::is_in_app(url.as_str()) {
        if policy::is_external_openable(url.as_str()) {
            let raw = url.to_string();
            let app = app.clone();
            let _ = app
                .clone()
                .run_on_main_thread(move || open_external(&app, &raw));
        }
        return NewWindowResponse::Deny;
    }
    if hand_off_call(app, parent_label, url.as_str()) {
        return NewWindowResponse::Deny;
    }
    if let Some((kind, _)) = accounts::document(url.as_str())
        && let Some(account_id) = account_id_from_label(parent_label)
    {
        open_tab_later(app, &account_id, url.as_str(), kind);
        return NewWindowResponse::Deny;
    }
    match popup_window(app, parent_label, session, url.clone(), features, identity) {
        Ok(window) => NewWindowResponse::Create { window },
        Err(err) => {
            log::warn!("in-app popup failed: {err}");
            if let Some(parent) = app.get_webview_window(parent_label) {
                let _ = parent.navigate(url);
            }
            NewWindowResponse::Deny
        }
    }
}

fn popup_window(
    app: &AppHandle,
    parent_label: &str,
    session: std::path::PathBuf,
    url: url::Url,
    features: tauri::webview::NewWindowFeatures,
    identity: BrowserIdentity,
) -> Result<WebviewWindow> {
    let label = format!("pop-{}", POPUP_SEQ.fetch_add(1, Ordering::Relaxed));
    let host = url.host_str().unwrap_or("popup").to_owned();
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .window_features(features)
        .title(format!("Workspaced — {host}"))
        .inner_size(960.0, 720.0)
        .data_directory(session.clone())
        .enable_clipboard_access()
        .zoom_hotkeys_enabled(true)
        .devtools(cfg!(debug_assertions));
    if let Some(user_agent) = accounts::user_agent(identity) {
        builder = builder.user_agent(user_agent);
    }
    builder = attach_guards(
        builder,
        app.clone(),
        parent_label.to_owned(),
        label.clone(),
        session,
        identity,
    );
    builder.build().context("build popup")
}

fn dispatch_sentinel(app: &AppHandle, window_label: &str, raw: &str) -> Result<()> {
    let (action, payload) = policy::sentinel(raw).context("invalid rail action")?;
    match action.as_str() {
        "focus" => {
            let id = if let Some(id) = payload.get("id").and_then(|value| value.as_str()) {
                id.to_owned()
            } else {
                let index = payload
                    .get("index")
                    .and_then(|value| value.as_u64())
                    .context("focus needs an account")? as usize;
                let state = app.state::<AppState>();
                let desks = lock(&state);
                desks.index_id(index).context("no account at that index")?
            };
            focus_account(app, &id, false)
        }
        "cycle" => {
            let delta = payload
                .get("delta")
                .and_then(|value| value.as_i64())
                .unwrap_or(1);
            let delta = i32::try_from(delta).unwrap_or(1);
            if !accept_action(&format!("cycle:{delta}")) {
                return Ok(());
            }
            let id = {
                let state = app.state::<AppState>();
                let mut desks = lock(&state);
                desks.cycle(delta)
            };
            let Some(id) = id else {
                return Ok(());
            };
            focus_account(app, &id, true)
        }
        "navigate" => {
            let suite = payload
                .get("suite")
                .and_then(|value| value.as_str())
                .context("missing suite app")?;
            let account_id = account_id_from_label(window_label)
                .context("navigation did not come from an account")?;
            open_under(app, &account_id, suite)?;
            Ok(())
        }
        "shell" => show_shell(app),
        "poll" => {
            // Only the feed view of a sleeping account may report. Pages
            // cannot set badges by pretending to be one.
            let (account_id, suite) = split_label(window_label).context("not an account view")?;
            if suite != crate::lifecycle::POLL_SUITE {
                anyhow::bail!("poll report from a page that is not a feed view");
            }
            let feed = serde_json::from_value(payload).context("feed report")?;
            crate::lifecycle::on_feed(app, account_id, feed);
            Ok(())
        }
        "poll-failed" => {
            let reason = payload
                .get("reason")
                .and_then(|reason| reason.as_str())
                .unwrap_or("unknown");
            log::warn!("unread check failed for {window_label}: {reason}");
            Ok(())
        }
        "close-tab" => {
            let (account_id, suite) = split_label(window_label).context("not an account view")?;
            if accounts::is_tab_id(suite) {
                close_tab(app, account_id, suite)?;
            }
            Ok(())
        }
        "settings" => {
            show_shell(app)?;
            let _ = app.emit("open-settings", ());
            Ok(())
        }
        "external" => {
            let url = payload
                .get("url")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            open_external(app, url);
            Ok(())
        }
        "rail" => {
            let collapsed = payload
                .get("collapsed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            {
                let state = app.state::<AppState>();
                let mut desks = lock(&state);
                desks.set_rail_collapsed(collapsed);
                desks.save()?;
            }
            deliver(app)
        }
        "quit" => {
            app.exit(0);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn open_external(app: &AppHandle, url: &str) {
    if !policy::is_external_openable(url) {
        return;
    }
    let host = url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned));
    if let Err(err) = app.opener().open_url(url, None::<&str>) {
        log::warn!("failed to open external link host={host:?}: {err}");
    }
}

fn install_shell_close(app: &AppHandle) -> Result<()> {
    let shell = app.get_window("shell").context("shell window missing")?;
    let app = app.clone();
    shell.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if any_account_visible(&app) {
                if let Some(shell) = app.get_webview_window("shell") {
                    let _ = shell.hide();
                }
            } else {
                app.exit(0);
            }
        }
    });
    Ok(())
}

fn any_account_visible(app: &AppHandle) -> bool {
    app.webview_windows()
        .into_iter()
        .any(|(label, window)| label.starts_with("acct-") && window.is_visible().unwrap_or(false))
}

fn rail_script(desks: &DeskState) -> String {
    let boot = serde_json::json!({
        "sentinel": format!("https://{}/rail", policy::SENTINEL_HOST),
        "suffixes": policy::PROPERTY_SUFFIXES,
        "styles": desks.page_styles(),
    });
    format!(
        "window.__WORKSPACED_BOOT={boot};{}",
        include_str!("rail.js")
    )
}

/// Restyles every open Google page after the palette or the setting changed.
pub fn push_page_styles(app: &AppHandle) {
    let styles = {
        let state = app.state::<AppState>();
        lock(&state).page_styles()
    };
    let Ok(json) = serde_json::to_string(&styles) else {
        return;
    };
    let script = format!("window.__workspacedStyles && window.__workspacedStyles({json})");
    for (label, webview) in app.webviews() {
        if label.starts_with("view-") {
            let _ = webview.eval(&script);
        }
    }
}
