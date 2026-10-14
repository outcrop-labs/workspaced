use std::cell::RefCell;

use anyhow::{Context, Result};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};

use std::sync::atomic::{AtomicI32, Ordering};

/// Sidebar widths. Keep in step with --rail in the frontend.
pub const RAIL_FULL: i32 = 300;
pub const RAIL_COMPACT: i32 = 68;

static RAIL_WIDTH: AtomicI32 = AtomicI32::new(RAIL_FULL);

fn rail_width() -> i32 {
    RAIL_WIDTH.load(Ordering::Relaxed)
}

/// Sets the sidebar width. Called once at startup before any layout exists,
/// and again whenever the user toggles the compact sidebar.
pub fn set_compact(app: &AppHandle, compact: bool) -> Result<()> {
    RAIL_WIDTH.store(
        if compact { RAIL_COMPACT } else { RAIL_FULL },
        Ordering::Relaxed,
    );
    #[cfg(target_os = "linux")]
    {
        finish_on_main(app, || {
            use gtk::prelude::*;
            if SETTINGS_OPEN.with(|open| open.get()) {
                // The rail stays wide until settings closes, but the views take
                // their new width now so nothing reflows as settings goes away.
                if let Some(row) = ROW.with(|slot| slot.borrow().clone()) {
                    row.queue_resize();
                }
                return;
            }
            if let Some(rail) = RAIL.with(|slot| slot.borrow().clone()) {
                rail.set_size_request(rail_width(), -1);
            }
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Ok(())
    }
}

thread_local! {
    static READY: RefCell<bool> = const { RefCell::new(false) };
}
#[cfg(target_os = "linux")]
thread_local! {
    static SETTINGS_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn show_surface(app: &AppHandle, label: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let label = label.to_owned();
        let shown = label.clone();
        finish_on_main(app, move || show_linux(&label))?;
        if let Some(webview) = app.get_webview(&shown) {
            let _ = webview.show();
            let _ = webview.set_focus();
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        show_bounds(app, label)
    }
}

pub fn show_settings(app: &AppHandle) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        finish_on_main(app, || apply_chrome(true))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn finish_on_main(app: &AppHandle, f: impl FnOnce() + Send + 'static) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        f();
        let _ = tx.send(());
    })
    .context("viewport main thread")?;
    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_chrome(settings: bool) {
    use gtk::prelude::*;
    let Some(rail) = RAIL.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let Some(row) = ROW.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let Some(views) = VIEWS.with(|slot| slot.borrow().clone()) else {
        return;
    };
    if SETTINGS_OPEN.with(|open| open.replace(settings) == settings) {
        return;
    }
    // The views are squeezed to nothing rather than hidden, so they stay
    // mapped and keep tracking the window size while settings is open.
    let pack = gtk::PackType::Start;
    row.set_child_packing(&rail, settings, true, 0, pack);
    row.set_child_packing(&views, !settings, true, 0, pack);
    // hexpand outranks the packing flag, so it has to follow too.
    rail.set_hexpand(settings);
    views.set_hexpand(!settings);
    rail.set_size_request(if settings { -1 } else { rail_width() }, -1);
}

pub fn adopt(shell: &tauri::Webview, webview: &Webview, label: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        ensure_linux_layout(shell)?;
        reparent(webview, label)?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (shell, webview, label);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn show_linux(label: &str) {
    use gtk::prelude::*;
    apply_chrome(false);
    let Some(views) = VIEWS.with(|slot| slot.borrow().clone()) else {
        log::warn!("app views are not ready");
        return;
    };
    let Some(child) = SURFACES.with(|map| map.borrow().get(label).cloned()) else {
        log::warn!("app surface {label} is not in the window");
        return;
    };
    let previous = FRONT.with(|front| front.replace(Some(label.to_owned())));
    child.show();
    place(&views, &child, true);
    if let Some(previous) = previous.filter(|previous| previous != label)
        && let Some(old) = SURFACES.with(|map| map.borrow().get(&previous).cloned())
    {
        place(&views, &old, false);
    }
}

#[cfg(not(target_os = "linux"))]
fn show_bounds(app: &AppHandle, label: &str) -> Result<()> {
    let shell = app
        .get_webview_window("shell")
        .context("viewport window missing")?;
    let size = shell.inner_size().context("viewport size")?;
    let width = size.width as f64;
    let height = size.height as f64;
    let content_x = rail_width() as f64;
    let content_w = (width - content_x).max(1.0);
    if let Some(sidebar) = app.get_webview("shell") {
        let _ = sidebar.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = sidebar.set_size(LogicalSize::new(content_x, height));
        let _ = sidebar.show();
    }
    for webview in shell.as_ref().window().webviews() {
        if webview.label() == "shell" {
            continue;
        }
        if webview.label() == label {
            let _ = webview.set_position(LogicalPosition::new(content_x, 0.0));
            let _ = webview.set_size(LogicalSize::new(content_w, height));
            let _ = webview.show();
            let _ = webview.set_focus();
        } else {
            let _ = webview.hide();
        }
    }
    let _ = shell.set_focus();
    Ok(())
}

// Layout on Linux, inside the shell window's own box:
//
//   Box [ rail webview (SIDEBAR) | views: GtkLayout ]
//
// WebKit defers resizing a view that is not mapped until it is mapped again,
// so a GtkStack (which unmaps hidden pages) made the first switch after a
// window resize reflow the page in front of the user. Instead every account
// view stays mapped inside `views` at the content size. The front one sits at
// the origin and the rest wait just past the right edge, where the layout
// clips them. A resize reaches all of them at once, and a switch only moves
// two widgets, so draw order and input never depend on stacking.
#[cfg(target_os = "linux")]
thread_local! {
    static VIEWS: RefCell<Option<gtk::Layout>> = const { RefCell::new(None) };
    static RAIL: RefCell<Option<gtk::Widget>> = const { RefCell::new(None) };
    static ROW: RefCell<Option<gtk::Box>> = const { RefCell::new(None) };
    static SURFACES: RefCell<std::collections::HashMap<String, gtk::Widget>> =
        RefCell::new(std::collections::HashMap::new());
    static FRONT: RefCell<Option<String>> = const { RefCell::new(None) };
    static CONTENT: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((0, 0)) };
}

/// Where a view goes: the origin when it is in front, otherwise just out of sight.
#[cfg(target_os = "linux")]
fn slot(front: bool) -> (i32, i32) {
    if front {
        (0, 0)
    } else {
        (CONTENT.with(|size| size.get().0).max(1) + 1, 0)
    }
}

#[cfg(target_os = "linux")]
fn place(views: &gtk::Layout, child: &gtk::Widget, front: bool) {
    use gtk::prelude::*;
    let (x, y) = slot(front);
    views.move_(child, x, y);
}

/// Sizes every view to the content area. Driven by the whole row, not by the
/// layout, so views keep the right size even while settings covers them.
#[cfg(target_os = "linux")]
fn resize_views(width: i32, height: i32) {
    use gtk::prelude::*;
    let size = ((width - rail_width()).max(1), height.max(1));
    if CONTENT.with(|content| content.replace(size)) == size {
        return;
    }
    let Some(views) = VIEWS.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let front = FRONT.with(|front| front.borrow().clone());
    SURFACES.with(|map| {
        for (label, child) in map.borrow().iter() {
            child.set_size_request(size.0, size.1);
            place(&views, child, front.as_deref() == Some(label.as_str()));
        }
    });
    views.queue_resize();
}

#[cfg(target_os = "linux")]
fn ensure_linux_layout(shell: &Webview) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = shell.with_webview(move |platform| {
        if READY.with(|ready| *ready.borrow()) {
            let _ = tx.send(());
            return;
        }
        use gtk::prelude::*;
        let view = platform.inner();
        let Some(parent) = view.parent() else {
            let _ = tx.send(());
            return;
        };
        let Ok(vbox) = parent.downcast::<gtk::Box>() else {
            let _ = tx.send(());
            return;
        };
        vbox.remove(&view);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.set_hexpand(true);
        row.set_vexpand(true);
        // WebKit asks for the width of its page, and the sidebar page is as wide
        // as its view, so a bare webview never gives width back: narrowed to the
        // compact rail it kept asking for 300px. A scrolled window with no
        // scrolling asks for exactly its size request and hands that to the view.
        let rail = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        rail.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
        rail.set_shadow_type(gtk::ShadowType::None);
        rail.set_propagate_natural_width(false);
        rail.set_size_request(rail_width(), -1);
        rail.set_hexpand(false);
        rail.set_vexpand(true);
        view.set_hexpand(true);
        view.set_vexpand(true);
        rail.add(&view);
        if let Some(viewport) = rail
            .child()
            .and_then(|child| child.downcast::<gtk::Viewport>().ok())
        {
            viewport.set_shadow_type(gtk::ShadowType::None);
        }
        row.pack_start(&rail, false, false, 0);
        let views = gtk::Layout::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        views.set_hexpand(true);
        views.set_vexpand(true);
        row.pack_start(&views, true, true, 0);
        // Resizing children inside an allocation pass is dropped by GTK, so the
        // views follow on the next main loop turn, before the frame is drawn.
        row.connect_size_allocate(|_, allocation| {
            let (width, height) = (allocation.width(), allocation.height());
            gtk::glib::idle_add_local_full(gtk::glib::Priority::HIGH_IDLE, move || {
                resize_views(width, height);
                gtk::glib::ControlFlow::Break
            });
        });
        vbox.pack_start(&row, true, true, 0);
        row.show_all();
        VIEWS.with(|slot| *slot.borrow_mut() = Some(views));
        RAIL.with(|slot| *slot.borrow_mut() = Some(rail.upcast()));
        ROW.with(|slot| *slot.borrow_mut() = Some(row));
        READY.with(|ready| *ready.borrow_mut() = true);
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    Ok(())
}

#[cfg(target_os = "linux")]
fn reparent(webview: &Webview, label: &str) -> Result<()> {
    let label = label.to_owned();
    let app = webview.app_handle().clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = webview.with_webview(move |platform| {
        use gtk::prelude::*;
        use webkit2gtk::SettingsExt;
        let view = platform.inner();
        if let Some(settings) = webkit2gtk::WebViewExt::settings(&view) {
            settings.set_enable_media(true);
            settings.set_enable_media_stream(true);
            settings.set_enable_mediasource(true);
            settings.set_enable_media_capabilities(true);
            settings.set_enable_webrtc(true);
            settings.set_enable_webaudio(true);
            settings.set_media_playback_allows_inline(true);
            settings.set_media_playback_requires_user_gesture(false);
            settings
                .set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Always);
        }
        let Some(views) = VIEWS.with(|slot| slot.borrow().clone()) else {
            let _ = tx.send(());
            return;
        };
        if let Some(parent) = view.parent()
            && let Ok(container) = parent.downcast::<gtk::Container>()
        {
            container.remove(&view);
        }
        let (width, height) = CONTENT.with(|size| size.get());
        if width > 0 && height > 0 {
            view.set_size_request(width, height);
        }
        let (x, y) = slot(false);
        views.put(&view, x, y);
        view.show();
        SURFACES.with(|map| {
            map.borrow_mut()
                .insert(label.clone(), view.clone().upcast())
        });
        instrument(&app, &view, &label);
        let gone = label;
        view.connect_destroy(move |_| {
            SURFACES.with(|map| map.borrow_mut().remove(&gone));
            FRONT.with(|front| {
                let mut front = front.borrow_mut();
                if front.as_deref() == Some(gone.as_str()) {
                    *front = None;
                }
            });
        });
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    Ok(())
}

fn surface_color(app: &AppHandle) -> tauri::webview::Color {
    let state = app.state::<crate::sessions::AppState>();
    let desks = crate::sessions::lock(&state);
    let (r, g, b) = crate::theme::rgb(&desks.theme().darker_background).unwrap_or((12, 14, 18));
    tauri::webview::Color(r, g, b, 255)
}

/// The browser session a view belongs to: one per account.
pub struct Session {
    /// Cookies and storage, kept apart from every other account.
    pub dir: std::path::PathBuf,
    pub identity: crate::accounts::BrowserIdentity,
    /// The script injected into every page.
    pub script: String,
    pub store_id: [u8; 16],
}

pub fn create_surface(
    app: &AppHandle,
    label: &str,
    url: url::Url,
    session: Session,
    parent_label: String,
) -> Result<Webview> {
    let Session {
        dir: session,
        identity,
        script,
        store_id,
    } = session;
    use tauri::webview::{PermissionKind, PermissionResponse, WebviewBuilder};
    std::fs::create_dir_all(&session).context("create session dir")?;
    let window = app.get_window("shell").context("viewport window missing")?;
    let shell = app
        .get_webview("shell")
        .context("sidebar webview missing")?;
    let size = window
        .inner_size()
        .unwrap_or(tauri::PhysicalSize::new(1400, 900));
    let mut builder = WebviewBuilder::new(label, WebviewUrl::External(url))
        .background_color(surface_color(app))
        .data_directory(session.clone())
        .data_store_identifier(store_id)
        .enable_clipboard_access()
        .zoom_hotkeys_enabled(true)
        .background_throttling(tauri_utils::config::BackgroundThrottlingPolicy::Throttle)
        .initialization_script(script)
        .on_download({
            let app = app.clone();
            move |_webview, event| {
                crate::sessions::on_download(&app, event);
                true
            }
        })
        .on_permission_request(|_webview, kind| match kind {
            PermissionKind::Microphone
            | PermissionKind::Camera
            | PermissionKind::DisplayCapture
            | PermissionKind::Notifications => PermissionResponse::Allow,
            _ => PermissionResponse::Default,
        });
    if let Some(user_agent) = crate::accounts::user_agent(identity) {
        builder = builder.user_agent(user_agent);
    }
    let app_nav = app.clone();
    let label_nav = label.to_owned();
    builder = builder.on_navigation(move |url| {
        crate::sessions::allow_navigation(&app_nav, &label_nav, url.as_str())
    });
    builder = track_page(builder, app, label);
    let app_pop = app.clone();
    let parent = parent_label.clone();
    let popup_session = session.clone();
    builder = builder.on_new_window(move |url, features| {
        crate::sessions::on_popup(
            &app_pop,
            &parent,
            popup_session.clone(),
            url,
            features,
            identity,
        )
    });
    #[cfg(target_os = "linux")]
    ensure_linux_layout(&shell)?;
    let webview = window
        .add_child(
            builder,
            LogicalPosition::new(rail_width() as f64, 0.0),
            LogicalSize::new(
                (size.width as f64 - rail_width() as f64).max(1.0),
                size.height as f64,
            ),
        )
        .context("add child webview")?;
    let _ = webview.hide();
    adopt(&shell, &webview, label)?;
    Ok(webview)
}

/// Linux-only extras for a Google view: notification clicks that switch to
/// their account, and spellcheck. Titles and load state are tracked for every
/// platform in `create_surface`. None of this gives the page any way to call
/// into the app.
#[cfg(target_os = "linux")]
fn instrument(app: &AppHandle, view: &webkit2gtk::WebView, label: &str) {
    use webkit2gtk::{NotificationExt, WebContextExt, WebViewExt};

    {
        let app = app.clone();
        let label = label.to_owned();
        view.connect_show_notification(move |_, notification| {
            let app = app.clone();
            let label = label.clone();
            notification
                .connect_clicked(move |_| crate::sessions::on_notification_clicked(&app, &label));
            // WebKit still shows it. This only adds the click behaviour.
            false
        });
    }

    if let Some(context) = view.context() {
        let lang = std::env::var("LANG").unwrap_or_default();
        let lang = lang
            .split('.')
            .next()
            .filter(|lang| !lang.is_empty() && *lang != "C")
            .unwrap_or("en_US");
        context.set_spell_checking_enabled(true);
        context.set_spell_checking_languages(&[lang]);
    }
}

/// Follows a view's title and loads on every platform: Gmail's title gives the
/// unread count and the signed-in address, a tab's title and address name it
/// and keep it current, and loads drive the spinners on the app icons.
fn track_page(
    builder: tauri::webview::WebviewBuilder<tauri::Wry>,
    app: &AppHandle,
    label: &str,
) -> tauri::webview::WebviewBuilder<tauri::Wry> {
    use tauri::Emitter;
    use tauri::webview::PageLoadEvent;
    let Some((account, suite)) = crate::sessions::split_label(label) else {
        return builder;
    };
    if suite == crate::lifecycle::POLL_SUITE {
        return builder;
    }
    let (account, suite) = (account.to_owned(), suite.to_owned());
    let tab = crate::accounts::is_tab_id(&suite);
    let titles = {
        let (app, account, suite) = (app.clone(), account.clone(), suite.clone());
        move |webview: Webview, title: String| {
            if suite == "mail" {
                crate::sessions::on_mail_title(&app, &account, &title);
            } else if tab {
                // In-page navigation changes the title too, so the address is read with it.
                let url = webview.url().ok().map(|url| url.to_string());
                crate::sessions::on_tab_page(&app, &account, &suite, Some(&title), url.as_deref());
            }
        }
    };
    let app = app.clone();
    builder
        .on_document_title_changed(titles)
        .on_page_load(move |_webview, payload| {
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            let _ = app.emit("surface-loading", (&account, &suite, loading));
            if tab && !loading {
                crate::sessions::on_tab_page(
                    &app,
                    &account,
                    &suite,
                    None,
                    Some(payload.url().as_str()),
                );
            }
        })
}
