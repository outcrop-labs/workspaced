//! Keeps only the accounts in use fully loaded.
//!
//! Each Gmail view costs most of a gigabyte. An account that has not been the
//! active one for a while is put to sleep: its views close and a small view
//! of Gmail's unread feed takes their place, reloaded every couple of minutes
//! for the unread count and new-mail notifications. Picking the account wakes
//! it, which closes the feed view and builds the account's views again.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use tauri::{AppHandle, Manager};

use crate::sessions::{self, AppState, lock};

/// The unread feed. It is a few kilobytes of XML, so its view stays small.
pub const FEED: &str = "https://mail.google.com/mail/u/0/feed/atom";
pub const POLL_SUITE: &str = "poll";
const POLL_EVERY: Duration = Duration::from_secs(120);
const TICK: Duration = Duration::from_secs(20);

#[derive(Default)]
struct Life {
    /// When each account stopped being the active one.
    left: HashMap<String, Instant>,
    asleep: HashSet<String>,
    polled: HashMap<String, Instant>,
    /// Message ids already shown, per account. Absent until the first poll,
    /// which only records what is there and announces nothing.
    seen: HashMap<String, HashSet<String>>,
}

static LIFE: Mutex<Option<Life>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Life) -> T) -> T {
    let mut guard = LIFE.lock().unwrap_or_else(|err| err.into_inner());
    f(guard.get_or_insert_with(Life::default))
}

pub fn is_asleep(id: &str) -> bool {
    with(|life| life.asleep.contains(id))
}

pub fn asleep() -> Vec<String> {
    with(|life| life.asleep.iter().cloned().collect())
}

/// The account stopped being the active one, so its sleep timer starts.
pub fn left(id: &str) {
    with(|life| {
        life.left.insert(id.to_owned(), Instant::now());
    });
}

/// Starts the timer that puts idle accounts to sleep and polls sleeping ones.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);
            if let Err(err) = tick(&app) {
                log::warn!("account upkeep failed: {err:#}");
            }
        }
    });
}

fn tick(app: &AppHandle) -> Result<()> {
    let (active, ids, after) = {
        let state = app.state::<AppState>();
        let desks = lock(&state);
        (
            desks.active_id().map(str::to_owned),
            desks
                .accounts()
                .iter()
                .map(|account| account.id.clone())
                .collect::<Vec<_>>(),
            desks.sleep_after(),
        )
    };
    for id in ids.iter().filter(|id| Some(*id) != active.as_ref()) {
        let (asleep, idle, due) = with(|life| {
            let idle = life.left.get(id).map(|at| at.elapsed());
            let due = life
                .polled
                .get(id)
                .is_none_or(|at| at.elapsed() >= POLL_EVERY);
            (life.asleep.contains(id), idle, due)
        });
        if asleep {
            if due {
                poll(app, id)?;
            }
        } else if let (Some(after), Some(idle)) = (after, idle)
            && idle >= after
        {
            sleep(app, id)?;
        }
    }
    Ok(())
}

/// Closes an account's views and leaves only its feed view watching for mail.
pub fn sleep(app: &AppHandle, id: &str) -> Result<()> {
    if with(|life| !life.asleep.insert(id.to_owned())) {
        return Ok(());
    }
    log::info!("account {id} is asleep");
    let prefix = format!("view-{id}-");
    for (label, webview) in app.webviews() {
        if label.starts_with(&prefix) && !label.ends_with(&format!("-{POLL_SUITE}")) {
            let _ = webview.close();
        }
    }
    poll(app, id)?;
    sessions::deliver(app)
}

/// Brings a sleeping account back. Its views are built again by whatever
/// shows it next.
pub fn wake(app: &AppHandle, id: &str) {
    if !with(|life| life.asleep.remove(id)) {
        return;
    }
    log::info!("account {id} is awake");
    with(|life| {
        life.polled.remove(id);
    });
    if let Some(webview) = app.get_webview(&sessions::surface_label(id, POLL_SUITE)) {
        let _ = webview.close();
    }
}

fn poll(app: &AppHandle, id: &str) -> Result<()> {
    with(|life| {
        life.polled.insert(id.to_owned(), Instant::now());
    });
    let label = sessions::surface_label(id, POLL_SUITE);
    match app.get_webview(&label) {
        Some(webview) => {
            let _ = webview.navigate(FEED.parse()?);
        }
        None => {
            sessions::open_under(app, id, POLL_SUITE)?;
            // Never shown. Hidden views still load and run their scripts.
            if let Some(webview) = app.get_webview(&label) {
                let _ = webview.hide();
            }
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct Feed {
    count: u32,
    email: Option<String>,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(serde::Deserialize)]
struct Entry {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    author: String,
}

/// A feed view reported in. Updates the badge and announces new mail.
pub fn on_feed(app: &AppHandle, id: &str, feed: Feed) {
    if !is_asleep(id) {
        return;
    }
    log::debug!("account {id} feed: {} unread", feed.count);
    let ids = feed
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<HashSet<_>>();
    let fresh = with(|life| {
        let first = !life.seen.contains_key(id);
        let seen = life.seen.entry(id.to_owned()).or_default();
        let fresh = feed
            .entries
            .iter()
            .filter(|entry| !seen.contains(&entry.id))
            .map(|entry| (entry.author.clone(), entry.title.clone()))
            .collect::<Vec<_>>();
        seen.extend(ids);
        if first { Vec::new() } else { fresh }
    });
    let label = {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        desks.set_unread(id, feed.count);
        if let Some(email) = feed.email.as_deref() {
            let _ = desks.set_email(id, email);
        }
        desks
            .accounts()
            .iter()
            .find(|account| account.id == id)
            .map(|account| account.label.clone())
            .unwrap_or_default()
    };
    let _ = sessions::deliver(app);
    if !fresh.is_empty() {
        announce(app, id, &label, &fresh);
    }
}

fn announce(app: &AppHandle, id: &str, account: &str, fresh: &[(String, String)]) {
    let (summary, body) = match fresh {
        [(author, title)] => (author.clone(), format!("{title}\n{account}")),
        _ => (format!("{} new messages", fresh.len()), account.to_owned()),
    };
    let mut note = notify_rust::Notification::new();
    note.appname("Workspaced")
        .summary(&summary)
        .body(&body)
        .icon("mail-unread");
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        note.action("default", "Open");
        let Ok(handle) = note.show() else {
            return;
        };
        let (app, id) = (app.clone(), id.to_owned());
        std::thread::spawn(move || {
            handle.wait_for_action(|action| {
                if action == "default" {
                    let _ = sessions::focus_surface(&app, &id, "mail")
                        .and_then(|_| sessions::show_shell(&app));
                }
            });
        });
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = (app, id);
        let _ = note.show();
    }
}
