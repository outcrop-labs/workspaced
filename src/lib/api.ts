import { invoke } from '@tauri-apps/api/core';

export type Account = {
	id: string;
	label: string;
	color: string;
	/** The Google address signed in, once Gmail has shown it. */
	email?: string;
	tabs?: Tab[];
};

export type Tab = {
	id: string;
	url: string;
	title: string;
	/** The suite app it belongs to, for its icon. */
	kind: string;
};

export type SuiteApp = {
	id: string;
	label: string;
	icon: string;
};

export type Theme = {
	name: string;
	mode: 'dark' | 'light' | string;
	font: string;
	background: string;
	darkBackground: string;
	darkerBackground: string;
	lighterBackground: string;
	foreground: string;
	darkForeground: string;
	brightForeground: string;
	accent: string;
	onAccent: string;
	muted: string;
	selection: string;
	red: string;
	/** Whether the palette came from an Omarchy theme. Google pages are only restyled then. */
	omarchy: boolean;
};

export type Snapshot = {
	accounts: Account[];
	activeId: string | null;
	browserIdentity: 'chrome' | 'system';
	railCollapsed: boolean;
	/** The app each account was last showing, by account id. Accounts not listed are on mail. */
	fronts: Record<string, string>;
	/** Unread inbox count by account id, for accounts whose Gmail has loaded. */
	unread: Record<string, number>;
	hiddenApps: string[];
	/** The browser that calls and Meet open in, if one is installed. */
	callBrowser: string | null;
	/** Whether Meet and calls run inside the app, which needs WebRTC in the webview. */
	callsNative: boolean;
	/** Accounts whose views are closed while a small feed view watches for mail. */
	asleep: string[];
	/** Minutes before an unused account sleeps, or null to keep every account loaded. */
	sleepAfter: number | null;
	pageTheme: PageTheme;
	suite: SuiteApp[];
	theme: Theme;
};

export function listAccounts(): Promise<Snapshot> {
	return invoke('list_accounts');
}

export function createAccount(label: string): Promise<Snapshot> {
	return invoke('create_account', { label });
}

export function renameAccount(id: string, label: string): Promise<Snapshot> {
	return invoke('rename_account', { id, label });
}

export function setAccountColor(id: string, color: string): Promise<Snapshot> {
	return invoke('set_account_color', { id, color });
}

export function removeAccount(id: string): Promise<Snapshot> {
	return invoke('remove_account', { id });
}

export function focusAccount(id: string): Promise<Snapshot> {
	return invoke('focus_account', { id });
}

export function moveAccount(id: string, delta: number): Promise<Snapshot> {
	return invoke('move_account', { id, delta });
}

export function cycleAccount(delta: number): Promise<Snapshot> {
	return invoke('cycle_account', { delta });
}

export function setBrowserIdentity(identity: Snapshot['browserIdentity']): Promise<Snapshot> {
	return invoke('set_browser_identity', { identity });
}

export function quit(): Promise<void> {
	return invoke('quit');
}

export type Surface = {
	accountId: string;
	suiteId: string;
};

export function listSurfaces(): Promise<Surface[]> {
	return invoke('list_surfaces');
}

export function openUnder(accountId: string, suiteId: string): Promise<void> {
	return invoke('open_under', { accountId, suiteId });
}

export function showSurface(accountId: string, suiteId: string): Promise<Snapshot> {
	return invoke('show_surface', { accountId, suiteId });
}

export function showSettings(): Promise<void> {
	return invoke('show_settings');
}

export function setRailCollapsed(collapsed: boolean): Promise<Snapshot> {
	return invoke('set_rail_collapsed', { collapsed });
}

export function setAppHidden(suiteId: string, hidden: boolean): Promise<Snapshot> {
	return invoke('set_app_hidden', { suiteId, hidden });
}

export function closeSurface(accountId: string, suiteId: string): Promise<Snapshot> {
	return invoke('close_surface', { accountId, suiteId });
}

export function reloadSurface(accountId: string, suiteId: string): Promise<void> {
	return invoke('reload_surface', { accountId, suiteId });
}

/** This page runs in the same webview engine as the Google pages, so its answer holds for them. */
export function reportCalls(): Promise<Snapshot> {
	return invoke('set_calls_native', { native: typeof RTCPeerConnection === 'function' });
}

export type PageTheme = { pages: boolean; documents: boolean };

export function setPageTheme(pageTheme: PageTheme): Promise<Snapshot> {
	return invoke('set_page_theme', { pageTheme });
}

export function setSleepAfter(minutes: number | null): Promise<Snapshot> {
	return invoke('set_sleep_after', { minutes });
}

export function revealDownload(path: string): Promise<void> {
	return invoke('reveal_download', { path });
}

export type Download = {
	url: string;
	name: string;
	path: string | null;
	status: 'started' | 'done' | 'failed';
};

export function newTab(accountId: string, kind: string): Promise<Snapshot> {
	return invoke('new_tab', { accountId, kind });
}

export function closeTab(accountId: string, tabId: string): Promise<Snapshot> {
	return invoke('close_tab', { accountId, tabId });
}

export function moveTab(accountId: string, tabId: string, delta: number): Promise<Snapshot> {
	return invoke('move_tab', { accountId, tabId, delta });
}
