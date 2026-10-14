//! Restyles Google's pages in the Omarchy palette.
//!
//! Google's current apps (Gmail, Calendar, Drive, Chat, Contacts, Keep, Tasks,
//! Meet, and the chrome of Docs, Sheets and Slides) draw their colours from
//! Material 3 custom properties, `--gm3-sys-color-*`, each with an `-rgb`
//! twin that state layers mix with alpha. Drive and Gmail also read the same
//! roles from `--dt-*` and `--pkw-*`. The apps declare those properties
//! again on dozens of their own components, under class names that change
//! with every release, so they are overridden on every element with
//! `!important`. That wins over every component declaration without naming
//! one.
//!
//! The pages of Docs and the grid of Sheets carry their colours inside the
//! drawing, where CSS colours cannot reach. They get a filter instead that
//! maps white to the theme's background and black to its text, as
//! DocsAfterDark does with a plain inversion. Only CSS filter functions are
//! used: WebKit draws those tiles as composited layers, and its compositor
//! ignores SVG `url(#…)` filters there.

use crate::theme::Theme;

/// What the page script needs: a stylesheet, and the SVG filter it uses.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Styles {
    pub css: String,
    /// Text colours for the page script's contrast guard, which recolours
    /// grey text Google painted with a literal colour that the theme turned
    /// unreadable. Empty when there is no theme.
    pub text: Option<Text>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Text {
    pub dark: bool,
    pub strong: String,
    pub soft: String,
    /// For near-white panels painted with literal colours on dark themes.
    pub surface: String,
}

type Rgb = (f32, f32, f32);

fn rgb(hex: &str) -> Rgb {
    let (r, g, b) = crate::theme::rgb(hex).unwrap_or((128, 128, 128));
    (f32::from(r), f32::from(g), f32::from(b))
}

/// `t` of the way from `a` to `b`.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn hex((r, g, b): Rgb) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        r.round() as u8,
        g.round() as u8,
        b.round() as u8
    )
}

fn triple((r, g, b): Rgb) -> String {
    format!(
        "{},{},{}",
        r.round() as u8,
        g.round() as u8,
        b.round() as u8
    )
}

/// The prefixes Google's apps use for the same colour roles: Material 3's
/// own, the design tokens Drive and Gmail read most of their surfaces from,
/// and a third set some shared components use.
const FAMILIES: &[&str] = &["gm3-sys-color-", "dt-", "pkw-"];

/// Every Material 3 colour role and the palette colour it takes.
fn roles(theme: &Theme) -> Vec<(&'static str, Rgb)> {
    let dark = theme.mode != "light";
    let bg = rgb(&theme.background);
    let fg = rgb(&theme.foreground);
    let soft = rgb(&theme.light_foreground);
    let accent = rgb(&theme.accent);
    let on_accent = rgb(&theme.on_accent);
    let red = rgb(&theme.red);
    let black = (0.0, 0.0, 0.0);
    // Containers step from the background toward the text colour, which
    // lightens them on dark themes and darkens them on light ones.
    let step = |t: f32| mix(bg, fg, t);
    let tint = |t: f32| mix(bg, accent, t);
    let lowest = if dark {
        rgb(&theme.darker_background)
    } else {
        bg
    };
    let secondary = mix(accent, fg, 0.45);
    let tertiary = mix(accent, fg, 0.25);
    vec![
        ("background", bg),
        ("on-background", fg),
        ("surface", bg),
        ("surface-dim", bg),
        ("surface-bright", step(0.16)),
        ("surface-container-lowest", lowest),
        ("surface-container-low", step(0.035)),
        ("surface-container", step(0.06)),
        ("surface-container-high", step(0.09)),
        ("surface-container-highest", step(0.13)),
        ("surface-variant", step(0.11)),
        ("on-surface", fg),
        ("on-surface-variant", soft),
        ("inverse-surface", fg),
        ("inverse-on-surface", bg),
        ("inverse-primary", accent),
        ("outline", step(0.42)),
        ("outline-variant", step(0.17)),
        ("primary", accent),
        ("surface-tint", accent),
        ("on-primary", on_accent),
        ("primary-container", tint(0.3)),
        ("on-primary-container", fg),
        ("primary-fixed", tint(0.3)),
        ("primary-fixed-dim", accent),
        ("on-primary-fixed", fg),
        ("on-primary-fixed-variant", fg),
        ("secondary", secondary),
        ("on-secondary", bg),
        ("secondary-container", tint(0.17)),
        ("on-secondary-container", fg),
        ("secondary-fixed", tint(0.17)),
        ("secondary-fixed-dim", secondary),
        ("on-secondary-fixed", fg),
        ("on-secondary-fixed-variant", fg),
        ("tertiary", tertiary),
        ("on-tertiary", bg),
        ("tertiary-container", tint(0.22)),
        ("on-tertiary-container", fg),
        ("tertiary-fixed", tint(0.22)),
        ("tertiary-fixed-dim", tertiary),
        ("on-tertiary-fixed", fg),
        ("on-tertiary-fixed-variant", fg),
        ("error", red),
        ("on-error", bg),
        ("error-container", mix(bg, red, 0.25)),
        ("on-error-container", fg),
        ("scrim", black),
        ("shadow", black),
    ]
}

/// The page styles for this palette, or none when the palette is not
/// Omarchy's or the user turned page theming off.
pub fn styles(theme: &Theme, pages: bool, documents: bool) -> Styles {
    if !theme.omarchy || !pages {
        return Styles::default();
    }
    let dark = theme.mode != "light";
    let bg = rgb(&theme.background);
    let fg = rgb(&theme.foreground);
    let mut css = String::from("html, html * {\n");
    for (role, colour) in roles(theme) {
        for family in FAMILIES {
            css.push_str(&format!(
                "  --{family}{role}: {} !important;\n  --{family}{role}-rgb: {} !important;\n",
                hex(colour),
                triple(colour)
            ));
        }
    }
    css.push_str("}\n");
    css.push_str(&format!(
        "html {{ color-scheme: {} !important; scrollbar-color: {} transparent; }}\n",
        if dark { "dark" } else { "light" },
        hex(mix(bg, fg, 0.25)),
    ));
    css.push_str(&format!(
        "::selection {{ background: {} !important; }}\n",
        hex(mix(bg, rgb(&theme.accent), 0.4))
    ));
    css.push_str(APPS);
    if documents {
        // White lands on the background and black on the text. Dark themes
        // invert first, rotating hues back so colours keep their meaning;
        // contrast and brightness then squeeze the range onto the theme's.
        // WebKit's compositor applies these filters in a space between sRGB
        // and linear light. An exponent of 1.6 was measured to land the page
        // on the theme background; 1 (sRGB) and 2.2 (linear) miss by a lot.
        let luma = |(r, g, b): Rgb| ((0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0).powf(1.6);
        let (page, ink) = (luma(bg), luma(fg));
        let (low, high) = if dark { (page, ink) } else { (ink, page) };
        let brightness = (low + high).max(0.01);
        let contrast = (high - low) / brightness;
        let invert = if dark {
            "invert(1) hue-rotate(180deg) "
        } else {
            ""
        };
        let filter = format!("{invert}contrast({contrast:.3}) brightness({brightness:.3})");
        css.push_str(
            &DOCUMENTS
                .replace("FILTER", &filter)
                .replace("PAGE", &hex(bg)),
        );
    }
    let text = Some(Text {
        dark,
        strong: theme.foreground.clone(),
        soft: theme.light_foreground.clone(),
        surface: hex(mix(bg, fg, 0.06)),
    });
    Styles { css, text }
}

/// Surfaces that take a fixed colour instead of a theme property. Kept to
/// stable, unscrambled names where Google offers them.
const APPS: &str = r#"
body { background-color: var(--gm3-sys-color-surface-container-low) !important; }
/* Gmail colour themes paint a fixed layer (.wl) and frosted panes over it
   (.bkK > .nH). Both are long-lived Gmail names, not scrambled ones. */
.wl { background: var(--gm3-sys-color-surface-container-low) !important; }
.bkK > .nH { background-color: var(--gm3-sys-color-surface) !important; }
"#;

/// Document pages in Docs and Sheets. Docs draws its pages as SVG tiles
/// (`.kix-canvas-tile-content`) and Sheets its grid on canvases; both take the
/// filter, which turns the page's white into the theme background. Only the
/// space around the pages is painted directly.
const DOCUMENTS: &str = r#"
.kix-canvas-tile-content, .kix-appview-editor canvas, .kix-cursor, #waffle-grid-container canvas, .grid-container canvas { filter: FILTER !important; }
.kix-appview-editor { background-color: PAGE !important; }
"#;
