//! Decides which navigations stay inside an account session.
//!
//! Google Workspace, its login flow, and the CDNs those apps load stay in the
//! webview. Everything else is cancelled and opened in the system browser.
//! `workspaced.invalid` is not a network host: the in-page rail navigates there
//! so the webview can signal the shell without giving Google pages IPC.

pub const SENTINEL_HOST: &str = "workspaced.invalid";

pub const PROPERTY_SUFFIXES: &[&str] = &[
    "googleusercontent.com",
    "gstatic.com",
    "googleapis.com",
    "ggpht.com",
    "googlemail.com",
    "youtube.com",
    "youtube-nocookie.com",
    "youtu.be",
    "ytimg.com",
    "googlevideo.com",
    "gvt1.com",
    "gvt2.com",
    "withgoogle.com",
    "recaptcha.net",
    "g.co",
    "goo.gl",
    // Google's sign-in and sign-out journeys route through its ad-attribution
    // hosts before landing back on a Google page. Left off this list they were
    // cancelled in the webview and handed to the system browser, where an
    // ad blocker kills the chain and the sign-in never arrives.
    "doubleclick.net",
    "googleadservices.com",
];

pub fn is_sentinel(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| host.eq_ignore_ascii_case(SENTINEL_HOST))
}

pub fn is_in_app(url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    match url.scheme() {
        "about" | "blob" | "data" => return true,
        "http" | "https" => {}
        _ => return false,
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    is_in_app_host(host)
}

pub fn is_external_openable(url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https" | "mailto") && !is_sentinel(url.as_str())
}

fn is_in_app_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || host == SENTINEL_HOST {
        return false;
    }
    if host == "google.com" || host.ends_with(".google.com") {
        return true;
    }
    if is_google_country_host(&host) {
        return true;
    }
    PROPERTY_SUFFIXES
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
}

fn is_google_country_host(host: &str) -> bool {
    let rest = if let Some(idx) = host.rfind(".google.") {
        &host[idx + ".google.".len()..]
    } else if let Some(rest) = host.strip_prefix("google.") {
        rest
    } else {
        return false;
    };
    is_country_tld(rest)
}

fn is_country_tld(tld: &str) -> bool {
    let mut parts = tld.split('.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(a), None, None) => is_cc(a),
        (Some(prefix), Some(cc), None) => {
            matches!(prefix, "co" | "com" | "org" | "net" | "ac" | "gov") && is_cc(cc)
        }
        _ => false,
    }
}

fn is_cc(label: &str) -> bool {
    label.len() == 2 && label.bytes().all(|b| b.is_ascii_lowercase())
}

/// Google's sign-in page, in any of the shapes a signed-out redirect takes.
pub fn is_sign_in(url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    if url.host_str() != Some("accounts.google.com") {
        return false;
    }
    url.path_segments().is_some_and(|mut parts| {
        matches!(
            parts.next(),
            Some("ServiceLogin" | "signin" | "AccountChooser")
        )
    })
}

/// The marketing pages a signed-out session gets dumped on. They carry no way
/// back to the app, so an account view that lands on one goes to the sign-in
/// page instead. Deliberate sign-out is not intercepted: its own flow has to
/// finish clearing the session first.
pub fn is_signed_out_marketing(url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "one.google.com" {
        return true;
    }
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host != "google.com" && !is_google_country_host(host) {
        return false;
    }
    let segments: Vec<_> = url
        .path_segments()
        .map(|parts| parts.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    segments.windows(2).any(|pair| pair == ["gmail", "about"])
}

/// The sign-in page for an account view, pointed back at the app it was
/// showing so the session lands where it started once signed in.
pub fn sign_in_url(continue_url: &str) -> String {
    let continue_url = url::Url::parse(continue_url)
        .ok()
        .filter(|url| is_in_app_host(url.host_str().unwrap_or_default()))
        .unwrap_or_else(|| url::Url::parse("https://mail.google.com/").unwrap());
    let mut url =
        url::Url::parse("https://accounts.google.com/ServiceLogin").expect("static url parses");
    url.query_pairs_mut()
        .append_pair("continue", continue_url.as_str());
    url.into()
}

pub fn sentinel(url: &str) -> Option<(String, serde_json::Value)> {
    let url = url::Url::parse(url).ok()?;
    if !url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case(SENTINEL_HOST))
    {
        return None;
    }
    let action = url
        .query_pairs()
        .find(|(key, _)| key == "a")?
        .1
        .into_owned();
    if action.is_empty() || action.len() > 32 {
        return None;
    }
    let payload = url
        .query_pairs()
        .find(|(key, _)| key == "p")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "{}".to_owned());
    let value = serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
    Some((action, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_open_as_tabs_and_frames_do_not() {
        use crate::accounts::document;
        let id = "1AbCdEfGhIjKlMnOp";
        assert_eq!(
            document(&format!("https://docs.google.com/document/d/{id}/edit")),
            Some(("docs", format!("docs:{id}")))
        );
        assert_eq!(
            document(&format!(
                "https://docs.google.com/spreadsheets/u/1/d/{id}/edit#gid=0"
            )),
            Some(("sheets", format!("sheets:{id}")))
        );
        assert_eq!(
            document(&format!("https://docs.google.com/presentation/d/{id}/view")),
            Some(("slides", format!("slides:{id}")))
        );
        assert_eq!(
            document(&format!("https://drive.google.com/file/d/{id}/view")),
            Some(("drive", format!("drive:{id}")))
        );
        assert_eq!(
            document("https://meet.google.com/abc-defg-hij"),
            Some(("meet", "meet:abc-defg-hij".into()))
        );
        assert_eq!(document("https://meet.google.com/_/frame"), None);
        assert!(crate::calls::is_call(
            "https://meet.google.com/abc-defg-hij?authuser=0"
        ));
        assert!(!crate::calls::is_call("https://meet.google.com.evil.com/x"));
        assert!(crate::calls::is_meeting(
            "https://meet.google.com/abc-defg-hij?authuser=1"
        ));
        assert!(crate::calls::is_meeting(
            "https://meet.google.com/lookup/abcdef"
        ));
        assert!(!crate::calls::is_meeting("https://meet.google.com/"));
        assert!(!crate::calls::is_meeting(
            "https://meet.google.com/_/frame?x=1"
        ));
        assert!(!crate::calls::is_meeting("https://meet.google.com/landing"));
        assert!(!crate::calls::is_meeting(
            "https://meet.google.com/hangouts/frame"
        ));
        assert_eq!(
            crate::calls::with_account(
                "https://meet.google.com/abc-defg-hij?authuser=0&hs=1",
                Some("me@example.com")
            )
            .unwrap(),
            "https://meet.google.com/abc-defg-hij?hs=1&authuser=me%40example.com"
        );
        assert!(crate::calls::with_account("https://evil.com/x", None).is_err());
        assert_eq!(
            document(&format!("https://drive.google.com/file/d/{id}/preview")),
            None
        );
        assert_eq!(
            document(&format!("https://docs.google.com/document/d/{id}/pub")),
            None
        );
        assert_eq!(document("https://docs.google.com/document/u/0/"), None);
        assert_eq!(document("https://meet.google.com/landing"), None);
        assert_eq!(document("https://mail.google.com/mail/u/0/"), None);
    }

    #[test]
    fn gmail_titles_give_inbox_count_and_address() {
        use crate::sessions::parse_gmail_title as parse;
        assert_eq!(
            parse("Inbox (383) - ada@example.com - Gmail"),
            (Some(383), Some("ada@example.com".into()))
        );
        assert_eq!(
            parse("Inbox (1,204) - ada@example.com - Example Mail"),
            (Some(1204), Some("ada@example.com".into()))
        );
        assert_eq!(
            parse("Inbox - ada@example.com - Gmail"),
            (Some(0), Some("ada@example.com".into()))
        );
        assert_eq!(
            parse("Sent Mail - ada@example.com - Gmail"),
            (None, Some("ada@example.com".into()))
        );
        assert_eq!(
            parse("Re: lunch - ada@example.com - Gmail"),
            (None, Some("ada@example.com".into()))
        );
        assert_eq!(parse("Gmail"), (None, None));
        assert_eq!(parse("Sign in - Google Accounts"), (None, None));
    }

    #[test]
    fn workspace_and_login_hosts_stay_in_app() {
        for url in [
            "https://mail.google.com/",
            "https://calendar.google.com/calendar/r",
            "https://drive.google.com/drive/my-drive",
            "https://docs.google.com/document/d/abc",
            "https://meet.google.com/lookup/abc",
            "https://chat.google.com/",
            "https://admin.google.com/",
            "https://accounts.google.com/ServiceLogin",
            "https://myaccount.google.com/",
            "https://workspace.google.com/",
            "https://ssl.gstatic.com/ui/v1/icons/mail/rfr/logo_gmail_lockup_default_1x_r5.png",
            "https://lh3.googleusercontent.com/a/photo",
            "https://www.googleapis.com/auth",
            "https://accounts.youtube.com/",
            "https://www.google.co.uk/",
            "https://www.google.com.au/",
            "https://google.de/",
            "http://mail.google.com/",
            "about:blank",
            "blob:https://mail.google.com/uuid",
            // The ad-attribution hops inside Google's sign-in and sign-out
            // journeys. They bounce back to a Google page; keeping them in
            // the app is what lets the journey finish.
            "https://googleads.g.doubleclick.net/pagead/conversion/123",
            "https://www.googleadservices.com/pagead/aclk?ai=1",
        ] {
            assert!(is_in_app(url), "{url} should stay in app");
        }
    }

    #[test]
    fn sign_in_pages_are_recognised() {
        for url in [
            "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fmail.google.com%2F",
            "https://accounts.google.com/AccountChooser?continue=x",
            "https://accounts.google.com/signin/v2/identifier",
        ] {
            assert!(is_sign_in(url), "{url} is a sign-in page");
        }
        for url in [
            "https://accounts.google.com/Logout",
            "https://myaccount.google.com/",
            "https://mail.google.com/",
            "https://accounts.evil.com/ServiceLogin",
        ] {
            assert!(!is_sign_in(url), "{url} is not a sign-in page");
        }
    }

    #[test]
    fn signed_out_marketing_is_recognised() {
        for url in [
            "https://www.google.com/gmail/about/",
            "https://www.google.com/intl/en_us/gmail/about/",
            "https://google.de/gmail/about/",
            "https://one.google.com/about/gmail",
        ] {
            assert!(is_signed_out_marketing(url), "{url} is marketing");
        }
        for url in [
            "https://www.google.com/",
            "https://www.google.com/search?q=gmail",
            "https://mail.google.com/gmail/about/",
            "https://workspace.google.com/",
            "https://accounts.google.com/ServiceLogin",
            "https://one.google.com.evil.com/",
        ] {
            assert!(!is_signed_out_marketing(url), "{url} is not marketing");
        }
    }

    #[test]
    fn sign_in_urls_point_back_at_the_app() {
        assert_eq!(
            sign_in_url("https://calendar.google.com/calendar/r"),
            "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fcalendar.google.com%2Fcalendar%2Fr"
        );
        // Anything not in-app falls back to mail, and garbage parses.
        assert!(sign_in_url("https://evil.com/").contains("mail.google.com"));
        assert!(sign_in_url("not a url").contains("mail.google.com"));
    }

    #[test]
    fn non_google_links_leave_the_app() {
        for url in [
            "https://example.com/",
            "https://google.evil.com/",
            "https://evilgoogle.com/",
            "https://notgoogle.com/",
            "https://mail.google.com.evil.com/",
            "https://github.com/google/workspace",
            "mailto:ada@example.com",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://workspaced.invalid/rail",
        ] {
            assert!(!is_in_app(url), "{url} should not stay in app");
        }
    }

    #[test]
    fn external_opener_rejects_script_and_sentinel_urls() {
        assert!(is_external_openable("https://example.com/a"));
        assert!(is_external_openable("mailto:ada@example.com"));
        assert!(!is_external_openable("javascript:alert(1)"));
        assert!(!is_external_openable("file:///tmp/x"));
        assert!(!is_external_openable(
            "https://workspaced.invalid/rail?a=quit"
        ));
    }

    #[test]
    fn sentinel_parses_rail_actions() {
        let (action, payload) =
            sentinel("https://workspaced.invalid/rail?a=focus&p=%7B%22index%22%3A1%7D")
                .expect("sentinel");
        assert_eq!(action, "focus");
        assert_eq!(payload["index"], 1);
        assert!(sentinel("https://mail.google.com/?a=focus").is_none());
    }
}
