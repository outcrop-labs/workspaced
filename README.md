# Workspaced

[![CI](https://github.com/outcrop-labs/workspaced/actions/workflows/ci.yml/badge.svg)](https://github.com/outcrop-labs/workspaced/actions/workflows/ci.yml)

Desktop app for keeping several Google Workspace accounts signed in at once.

Each account is an isolated session (its own cookies and storage) shown beside a sidebar that swaps between accounts and their Google apps. Links that are not Google open in the system browser.

```sh
pnpm install
pnpm tauri dev
```

Add an account, sign in, repeat. `Alt+1` through `Alt+9` jump to an account and `Alt+Down` / `Alt+Up` cycle, from the sidebar or from inside a Google page. The `…` on an account renames, recolors, reorders or removes it.

Each account shows its Gmail address and unread inbox count. Right-click an app icon to reload or close it, or to hide that app from the sidebar (Settings has the full list). The button at the bottom of the sidebar switches it to a compact, avatar-only rail. Downloads go to your Downloads folder and the sidebar offers to show each one. Inside a Google page, `Ctrl+R` reloads, `Alt+Left` / `Alt+Right` go back and forward, and `Ctrl+,` opens Settings.

Documents open as tabs under their account: a Doc, Sheet, Slides deck, Form, Drive file or Meet link clicked anywhere (Gmail, Drive, the Docs home list) lands in a tab in the sidebar, and opening one that is already open switches to it. **New** under the tabs starts a blank document. Tabs come back after a restart and load when clicked. `Ctrl+W` closes the tab you are in.

Calls and Meet run in the app wherever the webview has WebRTC: WebView2 on Windows, WKWebView on macOS, and WebKitGTK from 2.56, which is planned to ship libwebrtc. The app checks at startup and needs no setting. Distribution builds of WebKitGTK leave WebRTC out today, which is also why Gmail warns that calls may not work. There, meeting links open as an app window of Chromium (or Chrome, Brave, Edge, Vivaldi) with a separate profile per account; sign in there once per account. Sessions are not copied from the app, because Google signs out a session it sees in two browsers.

Accounts you are not using go to sleep after five minutes (Settings, Background accounts): their pages close and a small view of Gmail's unread feed checks every two minutes, keeping badges current and raising a notification for new mail. A Gmail view takes most of a gigabyte; a sleeping account takes about 60 MB. Picking a sleeping account loads it again. Only the active account loads at startup.

On Omarchy, the Google apps take the current theme's colors, and follow it when it changes. The pages' Material 3 color properties (`--gm3-sys-color-*`, `--dt-*`, `--pkw-*`) are overridden, Gmail's color-theme layer is neutralized, a contrast guard recolors grey text and white panels that are painted with fixed colors, and Docs and Sheets pages get an SVG filter mapping white to the theme background and black to its text (after DocsAfterDark's document inversion). Both can be switched off in Settings, Appearance.

Only one copy runs. Launching it again brings the window forward, and `workspaced mailto:ada@example.com` opens Gmail compose in the active account. Installed packages register it as a mail handler (`xdg-settings set default-url-scheme-handler mailto workspaced.desktop`). The window class is `workspaced` for compositor rules, and the title starts with the total unread count.

If Google refuses sign-in, switch Sign-in identity in Settings (the gear). Open apps reload with the new identity. Sessions stay on disk under the app data directory.

## Install

Builds for Linux (`.deb`, `.rpm`, AppImage), macOS (universal `.dmg`) and Windows (`.msi`, setup `.exe`) are attached to each [release](https://github.com/outcrop-labs/workspaced/releases). The macOS and Windows builds are not signed yet, so the first launch needs an explicit approval from the system.

## Development

```sh
pnpm install
pnpm tauri dev
```

On Linux this needs the WebKitGTK 4.1 development packages (`libwebkit2gtk-4.1-dev` on Debian and Ubuntu, `webkit2gtk-4.1` on Arch).

CI runs the type check, the frontend build, `cargo fmt`, clippy (warnings are errors) and the tests on Linux, macOS and Windows.

## Releases

Versions come from commit messages, in the [Conventional Commits](https://www.conventionalcommits.org) style: `fix:` for a fix, `feat:` for a feature, `feat!:` or a `BREAKING CHANGE:` footer for a breaking one. Each push to `main` updates a release pull request with the next version and its changelog. Merging it tags the release and builds the installers for every platform.

## Licence

MIT. See [LICENSE](LICENSE).
