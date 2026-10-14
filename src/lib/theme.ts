import type { Theme } from '$lib/api';

const fields: Array<[string, Exclude<keyof Theme, 'omarchy'>]> = [
	['--bg', 'background'],
	['--bg-dark', 'darkBackground'],
	['--bg-darker', 'darkerBackground'],
	['--bg-light', 'lighterBackground'],
	['--fg', 'foreground'],
	['--fg-dim', 'darkForeground'],
	['--fg-bright', 'brightForeground'],
	['--accent', 'accent'],
	['--on-accent', 'onAccent'],
	['--muted', 'muted'],
	['--selection', 'selection'],
	['--danger', 'red']
];

// app.html replays this before first paint so a restart does not flash the default colors.
const CACHE = 'workspaced:theme';

export function applyTheme(theme: Theme | undefined) {
	if (!theme) return;
	const font = theme.font.replaceAll('"', '').trim() || 'Adwaita Sans';
	const vars: Record<string, string> = {
		'color-scheme': theme.mode === 'light' ? 'light' : 'dark',
		'font-family': `"${font}", "Adwaita Sans", sans-serif`
	};
	for (const [name, key] of fields) vars[name] = theme[key];
	const root = document.documentElement;
	for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
	try {
		localStorage.setItem(CACHE, JSON.stringify(vars));
	} catch {
		// Storage can be unavailable. The theme still applies for this run.
	}
}
