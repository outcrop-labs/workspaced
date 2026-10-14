//! Where calls and Meet run.
//!
//! When the webview has WebRTC (WebView2 on Windows, WKWebView on macOS, and
//! WebKitGTK once it ships libwebrtc, planned for 2.56) Meet runs in the app
//! like any other Google page. The shell page tests this and reports it.
//!
//! Without WebRTC (WebKitGTK as distributions build it today) Meet links open
//! as app windows of a Chromium-family browser, each account with its own
//! profile under the app data directory, so accounts stay as separate there
//! as they are here.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};

static NATIVE: AtomicBool = AtomicBool::new(false);

/// Records whether the webview can make calls. Until the shell page has said
/// so, calls are assumed not to work in the app.
pub fn set_native(native: bool) {
    NATIVE.store(native, Ordering::Relaxed);
}

/// Whether Meet runs in the app.
pub fn native() -> bool {
    NATIVE.load(Ordering::Relaxed)
}

/// Whether calls leave the app for a browser.
pub fn external() -> bool {
    !native() && browser().is_some()
}

/// Browsers that run Meet, best first, as executables looked up on PATH.
#[cfg(target_os = "linux")]
const BROWSERS: &[(&str, &str)] = &[
    ("chromium", "Chromium"),
    ("google-chrome-stable", "Google Chrome"),
    ("google-chrome", "Google Chrome"),
    ("brave", "Brave"),
    ("brave-browser", "Brave"),
    ("microsoft-edge-stable", "Microsoft Edge"),
    ("vivaldi-stable", "Vivaldi"),
];

/// Browsers that run Meet, best first, as paths inside their app bundles.
#[cfg(target_os = "macos")]
const BROWSERS: &[(&str, &str)] = &[
    (
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "Google Chrome",
    ),
    (
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "Chromium",
    ),
    (
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "Brave",
    ),
    (
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "Microsoft Edge",
    ),
];

/// Browsers that run Meet, best first, under Program Files.
#[cfg(target_os = "windows")]
const BROWSERS: &[(&str, &str)] = &[
    (r"Google\Chrome\Application\chrome.exe", "Google Chrome"),
    (r"Microsoft\Edge\Application\msedge.exe", "Microsoft Edge"),
    (
        r"BraveSoftware\Brave-Browser\Application\brave.exe",
        "Brave",
    ),
];

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
const BROWSERS: &[(&str, &str)] = &[];

/// The browser calls open in when they cannot run in the app.
pub fn browser() -> Option<(PathBuf, &'static str)> {
    BROWSERS.iter().find_map(|(binary, name)| {
        candidates(binary)
            .into_iter()
            .find(|candidate| candidate.is_file())
            .map(|found| (found, *name))
    })
}

fn candidates(binary: &str) -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|dir| PathBuf::from(dir).join(binary))
            .collect()
    } else if cfg!(target_os = "macos") {
        vec![PathBuf::from(binary)]
    } else {
        std::env::var_os("PATH")
            .map(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join(binary))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Any Meet page, including the internal ones Google pages load in frames.
pub fn is_call(raw: &str) -> bool {
    url::Url::parse(raw)
        .is_ok_and(|url| url.scheme() == "https" && url.host_str() == Some("meet.google.com"))
}

/// A link to an actual meeting: a meeting code, a lookup link or a new
/// meeting. Only these leave the app on their own; Meet's internal pages,
/// which Gmail and Calendar load in the background, never do.
pub fn is_meeting(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https" || url.host_str() != Some("meet.google.com") {
        return false;
    }
    let mut parts = url
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty());
    match parts.next() {
        Some("lookup") | Some("new") => true,
        Some(code) => {
            code.split('-').map(str::len).eq([3, 4, 3])
                && code.chars().all(|ch| ch == '-' || ch.is_ascii_lowercase())
        }
        None => false,
    }
}

/// Opens a call as an app window in the account's browser profile. The first
/// time, the user signs in there once. Sessions are not copied over from the
/// app: Google treats one session used by two browsers as stolen and signs it
/// out, in the app as well.
pub fn open(profiles: &Path, account_id: &str, url: &str) -> Result<()> {
    if !crate::accounts::is_safe_id(account_id) {
        anyhow::bail!("invalid account id");
    }
    if !is_call(url) {
        anyhow::bail!("not a Meet link");
    }
    let (binary, _) = browser().context("install Chromium or Google Chrome to make calls")?;
    let profile = profiles.join(account_id);
    std::fs::create_dir_all(&profile).context("create call profile")?;
    Command::new(binary)
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--app={url}"))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        // Distribution-installed extensions (1Password here) open welcome windows in every new profile.
        .arg("--disable-extensions")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("start the browser for the call")?;
    Ok(())
}
