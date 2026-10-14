#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::accounts::DeskState;
use crate::sessions::{self, AppState, lock};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub name: String,
    pub mode: String,
    pub font: String,
    pub background: String,
    pub dark_background: String,
    pub darker_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub dark_foreground: String,
    pub bright_foreground: String,
    /// Secondary text. Softer than foreground, brighter than dark_foreground.
    pub light_foreground: String,
    pub accent: String,
    pub on_accent: String,
    pub muted: String,
    pub selection: String,
    pub red: String,
    /// Whether this palette came from an Omarchy theme. Google pages are only
    /// restyled for Omarchy, where the palette is the user's own.
    pub omarchy: bool,
}

impl Theme {
    pub fn load() -> Self {
        #[cfg(target_os = "linux")]
        {
            if let Some(theme) = load_omarchy() {
                return theme;
            }
        }
        Self::fallback()
    }

    fn fallback() -> Self {
        Self::from_map(
            "Default",
            "Adwaita Sans",
            &[
                ("mode", "dark"),
                ("background", "#16181d"),
                ("dark_background", "#101216"),
                ("darker_background", "#0c0e12"),
                ("lighter_background", "#22262e"),
                ("foreground", "#e7e4df"),
                ("dark_foreground", "#9a958c"),
                ("bright_foreground", "#f7f4ef"),
                ("accent", "#e8a54b"),
                ("muted", "#343a44"),
                ("selection", "#2a241c"),
                ("red", "#d4534a"),
            ],
        )
    }

    fn from_map(name: &str, font: &str, pairs: &[(&str, &str)]) -> Self {
        let get = |key: &str, fallback: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
                .unwrap_or_else(|| fallback.to_owned())
        };
        let accent = get("accent", "#e8a54b");
        let on_accent = contrasting(&accent);
        Self {
            name: name.to_owned(),
            mode: get("mode", "dark"),
            font: font.to_owned(),
            background: get("background", "#16181d"),
            dark_background: get("dark_background", "#101216"),
            darker_background: get("darker_background", "#0c0e12"),
            lighter_background: get("lighter_background", "#22262e"),
            foreground: get("foreground", "#e7e4df"),
            dark_foreground: get("dark_foreground", "#9a958c"),
            bright_foreground: get("bright_foreground", "#f7f4ef"),
            light_foreground: get("light_foreground", "#bdb8b0"),
            accent: accent.clone(),
            on_accent,
            muted: get("muted", "#343a44"),
            selection: get("selection", "#2a241c"),
            red: get("red", "#d4534a"),
            omarchy: false,
        }
    }
}

pub fn apply(app: &AppHandle) {
    let theme = Theme::load();
    {
        let state = app.state::<AppState>();
        let mut desks = lock(&state);
        if desks.theme() == &theme {
            return;
        }
        desks.set_theme(theme.clone());
    }
    paint_shell(app, &theme);
    let _ = app.emit("theme-updated", &theme);
    sessions::push_page_styles(app);
    if let Err(err) = sessions::deliver(app) {
        log::warn!("theme broadcast failed: {err}");
    }
}

pub fn watch(app: AppHandle) {
    #[cfg(target_os = "linux")]
    {
        let Some(dir) = omarchy_current_dir() else {
            return;
        };
        std::thread::spawn(move || {
            if let Err(err) = watch_dir(app, dir) {
                log::warn!("theme watch failed: {err}");
            }
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "linux")]
fn watch_dir(app: AppHandle, dir: PathBuf) -> Result<(), notify::Error> {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    if dir.exists() {
        watcher.watch(&dir, RecursiveMode::Recursive)?;
    }
    let mut pending = false;
    loop {
        let event = if pending {
            match rx.recv_timeout(Duration::from_millis(180)) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    pending = false;
                    let app = app.clone();
                    let _ = app.clone().run_on_main_thread(move || apply(&app));
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(event) => event,
                Err(_) => break,
            }
        };
        if event_is_theme(event) {
            pending = true;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn event_is_theme(event: Result<notify::Event, notify::Error>) -> bool {
    let Ok(event) = event else {
        return false;
    };
    if !matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
    ) {
        return false;
    }
    event.paths.iter().any(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        name == "colors.toml" || name == "theme.name"
    })
}

fn paint_shell(app: &AppHandle, theme: &Theme) {
    let Some(shell) = app.get_window("shell") else {
        return;
    };
    let Some((r, g, b)) = rgb(&theme.darker_background) else {
        return;
    };
    let _ = shell.set_background_color(Some(tauri::window::Color(r, g, b, 255)));
}

#[cfg(target_os = "linux")]
fn load_omarchy() -> Option<Theme> {
    let dir = omarchy_current_dir()?.join("theme");
    let raw = std::fs::read_to_string(dir.join("colors.toml")).ok()?;
    let pairs = parse_toml_pairs(&raw);
    let name = std::fs::read_to_string(omarchy_current_dir()?.join("theme.name"))
        .unwrap_or_default()
        .trim()
        .to_owned();
    let name = if name.is_empty() {
        "Omarchy".to_owned()
    } else {
        name
    };
    let lookup = |key: &str, fallback: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| fallback.to_owned())
    };
    let accent = lookup("accent", "#e8a54b");
    Some(Theme {
        name,
        mode: lookup("mode", "dark"),
        font: desktop_font(),
        background: lookup("background", "#16181d"),
        dark_background: lookup("dark_background", "#101216"),
        darker_background: lookup("darker_background", "#0c0e12"),
        lighter_background: lookup("lighter_background", "#22262e"),
        foreground: lookup("foreground", "#e7e4df"),
        dark_foreground: lookup("dark_foreground", "#9a958c"),
        bright_foreground: lookup("bright_foreground", "#f7f4ef"),
        light_foreground: lookup("light_foreground", &lookup("foreground", "#e7e4df")),
        on_accent: contrasting(&accent),
        accent,
        muted: lookup("muted", "#343a44"),
        selection: lookup("selection", "#2a241c"),
        red: lookup("red", "#d4534a"),
        omarchy: true,
    })
}

#[cfg(target_os = "linux")]
fn omarchy_current_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".local/state/omarchy/current"))
}

#[cfg(target_os = "linux")]
fn desktop_font() -> String {
    static FONT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "font-name"])
            .output()
        else {
            return "Adwaita Sans".to_owned();
        };
        let text = String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_owned();
        let name = text
            .rsplit_once(' ')
            .filter(|(_, size)| size.chars().all(|ch| ch.is_ascii_digit()))
            .map(|(name, _)| name.to_owned())
            .unwrap_or(text);
        if name.is_empty() {
            "Adwaita Sans".to_owned()
        } else {
            name
        }
    });
    FONT.clone()
}

#[cfg(target_os = "linux")]
fn parse_toml_pairs(raw: &str) -> Vec<(String, String)> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim().trim_matches('"').trim_matches('\'');
            Some((key.trim().to_owned(), value.to_owned()))
        })
        .collect()
}

fn contrasting(hex: &str) -> String {
    if luminance(hex) > 0.62 {
        "#101418".to_owned()
    } else {
        "#f7f4ef".to_owned()
    }
}

fn luminance(hex: &str) -> f32 {
    let Some((r, g, b)) = rgb(hex) else {
        return 0.0;
    };
    let channel = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

pub(crate) fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

pub fn install(desks: &mut DeskState) {
    desks.set_theme(Theme::load());
}
