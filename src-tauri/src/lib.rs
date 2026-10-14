mod accounts;
mod calls;
mod icons;
mod lifecycle;
mod pages;
mod policy;
mod sessions;
mod theme;
mod viewport;
use anyhow::Context;
use tauri::{AppHandle, Manager, State};

use accounts::{BrowserIdentity, Snapshot};
use sessions::{AppState, lock};

fn err(result: anyhow::Result<Snapshot>) -> Result<Snapshot, String> {
    result.map_err(|err| err.to_string())
}

#[tauri::command]
fn list_accounts(state: State<'_, AppState>) -> Snapshot {
    lock(&state).snapshot()
}

#[tauri::command]
fn create_account(
    app: AppHandle,
    state: State<'_, AppState>,
    label: String,
) -> Result<Snapshot, String> {
    err((|| {
        let id = {
            let mut desks = lock(&state);
            desks.create(&label)?.id
        };
        sessions::focus_account(&app, &id, true)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn rename_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    label: String,
) -> Result<Snapshot, String> {
    err((|| {
        {
            let mut desks = lock(&state);
            desks.rename(&id, &label)?;
        }
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn set_account_color(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    color: String,
) -> Result<Snapshot, String> {
    err((|| {
        {
            let mut desks = lock(&state);
            desks.set_color(&id, &color)?;
        }
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn remove_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::remove_account(&app, &id)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn focus_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::focus_account(&app, &id, false)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn focus_shell(app: AppHandle) -> Result<(), String> {
    sessions::show_shell(&app).map_err(|err| err.to_string())
}

#[tauri::command]
fn move_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    delta: i32,
) -> Result<Snapshot, String> {
    err((|| {
        {
            let mut desks = lock(&state);
            desks.move_by(&id, delta)?;
        }
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn cycle_account(
    app: AppHandle,
    state: State<'_, AppState>,
    delta: i32,
) -> Result<Snapshot, String> {
    err((|| {
        if delta == 0 {
            return Ok(lock(&state).snapshot());
        }
        let id = {
            let mut desks = lock(&state);
            desks.cycle(delta)
        };
        let Some(id) = id else {
            return Ok(lock(&state).snapshot());
        };
        sessions::focus_account(&app, &id, true)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn set_browser_identity(
    app: AppHandle,
    state: State<'_, AppState>,
    identity: String,
) -> Result<Snapshot, String> {
    err((|| {
        let identity = match identity.as_str() {
            "chrome" => BrowserIdentity::Chrome,
            "system" => BrowserIdentity::System,
            _ => anyhow::bail!("unknown browser identity"),
        };
        sessions::set_identity(&app, identity)?;
        Ok(lock(&state).snapshot())
    })())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Surface {
    account_id: String,
    suite_id: String,
}

#[tauri::command]
fn list_surfaces(app: AppHandle) -> Vec<Surface> {
    app.webviews()
        .into_iter()
        .filter_map(|(_, webview)| {
            let label = webview.label();
            let rest = label.strip_prefix("view-")?;
            let (account_id, suite_id) = rest.rsplit_once('-')?;
            if suite_id == lifecycle::POLL_SUITE {
                return None;
            }
            Some(Surface {
                account_id: account_id.to_owned(),
                suite_id: suite_id.to_owned(),
            })
        })
        .collect()
}

#[tauri::command]
fn open_under(app: AppHandle, account_id: String, suite_id: String) -> Result<(), String> {
    sessions::open_under(&app, &account_id, &suite_id).map_err(|error| format!("{error:#}"))
}

#[tauri::command]
fn show_surface(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    suite_id: String,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::focus_surface(&app, &account_id, &suite_id)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    crate::viewport::show_settings(&app).map_err(|error| format!("{error:#}"))
}

#[tauri::command]
fn set_rail_collapsed(
    app: AppHandle,
    state: State<'_, AppState>,
    collapsed: bool,
) -> Result<Snapshot, String> {
    err((|| {
        {
            let mut desks = lock(&state);
            desks.set_rail_collapsed(collapsed);
            desks.save()?;
        }
        viewport::set_compact(&app, collapsed)?;
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn set_app_hidden(
    app: AppHandle,
    state: State<'_, AppState>,
    suite_id: String,
    hidden: bool,
) -> Result<Snapshot, String> {
    err((|| {
        lock(&state).set_app_hidden(&suite_id, hidden)?;
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn close_surface(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    suite_id: String,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::close_surface(&app, &account_id, &suite_id)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn reload_surface(app: AppHandle, account_id: String, suite_id: String) -> Result<(), String> {
    sessions::reload_surface(&app, &account_id, &suite_id).map_err(|error| format!("{error:#}"))
}

#[tauri::command]
fn reveal_download(app: AppHandle, path: String) -> Result<(), String> {
    sessions::reveal_download(&app, &path).map_err(|error| format!("{error:#}"))
}

#[tauri::command]
fn new_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    kind: String,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::new_tab(&app, &account_id, &kind)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn close_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    tab_id: String,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::close_tab(&app, &account_id, &tab_id)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn move_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    tab_id: String,
    delta: i32,
) -> Result<Snapshot, String> {
    err((|| {
        sessions::move_tab(&app, &account_id, &tab_id, delta)?;
        Ok(lock(&state).snapshot())
    })())
}

/// The shell page reports whether the webview can make calls. It runs in the
/// same engine as the Google pages, so its answer holds for them too.
#[tauri::command]
fn set_calls_native(
    app: AppHandle,
    state: State<'_, AppState>,
    native: bool,
) -> Result<Snapshot, String> {
    calls::set_native(native);
    log::info!(
        "calls run {}",
        if native { "in the app" } else { "in a browser" }
    );
    err((|| {
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

/// Minutes before an unused account sleeps, or None to keep them all loaded.
#[tauri::command]
fn set_sleep_after(
    app: AppHandle,
    state: State<'_, AppState>,
    minutes: Option<u32>,
) -> Result<Snapshot, String> {
    err((|| {
        lock(&state).set_sleep_after(minutes)?;
        if minutes.is_none() {
            // Keeping everything loaded: wake the sleepers and load their mail.
            let asleep = lifecycle::asleep();
            for id in &asleep {
                lifecycle::wake(&app, id);
            }
            let warm = app.clone();
            std::thread::spawn(move || sessions::warm_accounts(&warm, &asleep));
        }
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn set_page_theme(
    app: AppHandle,
    state: State<'_, AppState>,
    page_theme: accounts::PageTheme,
) -> Result<Snapshot, String> {
    err((|| {
        lock(&state).set_page_theme(page_theme)?;
        sessions::push_page_styles(&app);
        sessions::deliver(&app)?;
        Ok(lock(&state).snapshot())
    })())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first, so a second launch exits before it opens any sessions.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            sessions::on_launch(app, &args);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let dir = app.path().app_data_dir().context("app data dir")?;
            let mut desks = accounts::DeskState::load(&dir)?;
            theme::install(&mut desks);
            let compact = desks.rail_collapsed();
            app.manage(AppState {
                desks: std::sync::Mutex::new(desks),
            });
            theme::apply(app.handle());
            viewport::set_compact(app.handle(), compact)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    if handle.get_window("shell").is_some() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if let Err(error) = sessions::restore(&handle) {
                    log::error!("restore failed: {error:#}");
                }
                let args = std::env::args().skip(1).collect::<Vec<_>>();
                if args
                    .iter()
                    .any(|arg| arg.to_ascii_lowercase().starts_with("mailto:"))
                {
                    sessions::on_launch(&handle, &args);
                }
            });
            theme::watch(app.handle().clone());
            lifecycle::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            create_account,
            rename_account,
            set_account_color,
            remove_account,
            focus_account,
            focus_shell,
            move_account,
            cycle_account,
            set_browser_identity,
            list_surfaces,
            open_under,
            show_surface,
            show_settings,
            set_rail_collapsed,
            set_app_hidden,
            close_surface,
            reload_surface,
            reveal_download,
            new_tab,
            close_tab,
            move_tab,
            set_calls_native,
            set_sleep_after,
            set_page_theme,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
