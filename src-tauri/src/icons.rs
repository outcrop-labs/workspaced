use std::collections::HashMap;
use std::sync::LazyLock;

fn png(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", b64(bytes))
}

fn b64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | bytes[i + 2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    if i < bytes.len() {
        let mut n = (bytes[i] as u32) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        if i + 1 < bytes.len() {
            n |= (bytes[i + 1] as u32) << 8;
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
    }
    out
}

pub fn service_icon(id: &str) -> String {
    ICONS.get(id).cloned().unwrap_or_default()
}

static ICONS: LazyLock<HashMap<&'static str, String>> = LazyLock::new(|| {
    HashMap::from([
        ("mail", png(include_bytes!("../assets/services/mail.png"))),
        (
            "calendar",
            png(include_bytes!("../assets/services/calendar.png")),
        ),
        ("drive", png(include_bytes!("../assets/services/drive.png"))),
        ("docs", png(include_bytes!("../assets/services/docs.png"))),
        (
            "sheets",
            png(include_bytes!("../assets/services/sheets.png")),
        ),
        (
            "slides",
            png(include_bytes!("../assets/services/slides.png")),
        ),
        ("meet", png(include_bytes!("../assets/services/meet.png"))),
        ("chat", png(include_bytes!("../assets/services/chat.png"))),
        (
            "contacts",
            png(include_bytes!("../assets/services/contacts.png")),
        ),
        ("keep", png(include_bytes!("../assets/services/keep.png"))),
        ("tasks", png(include_bytes!("../assets/services/tasks.png"))),
        ("forms", png(include_bytes!("../assets/services/forms.png"))),
        ("sites", png(include_bytes!("../assets/services/sites.png"))),
        (
            "groups",
            png(include_bytes!("../assets/services/groups.png")),
        ),
        ("admin", png(include_bytes!("../assets/services/admin.png"))),
    ])
});
