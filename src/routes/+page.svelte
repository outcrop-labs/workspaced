<script lang="ts">
	import { listen } from '@tauri-apps/api/event';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Check from '@lucide/svelte/icons/check';
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import Download from '@lucide/svelte/icons/download';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import Layers from '@lucide/svelte/icons/layers';
	import Link from '@lucide/svelte/icons/link';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
	import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Plus from '@lucide/svelte/icons/plus';
	import RotateCw from '@lucide/svelte/icons/rotate-cw';
	import Settings from '@lucide/svelte/icons/settings';
	import X from '@lucide/svelte/icons/x';
	import { onMount, tick } from 'svelte';
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { fade, fly, slide } from 'svelte/transition';
	import {
		closeSurface,
		closeTab,
		moveTab,
		newTab,
		createAccount,
		cycleAccount,
		focusAccount,
		listAccounts,
		listSurfaces,
		moveAccount,
		quit,
		reloadSurface,
		removeAccount,
		renameAccount,
		reportCalls,
		revealDownload,
		setAccountColor,
		setAppHidden,
		setBrowserIdentity,
		setRailCollapsed,
		setSleepAfter,
		setPageTheme,
		showSettings,
		showSurface,
		type Account,
		type Download as DownloadEvent,
		type Snapshot,
		type SuiteApp,
		type Tab,
		type Surface,
		type Theme
	} from '$lib/api';
	import AccountRow from '$lib/components/AccountRow.svelte';
	import Menu, { type MenuRequest } from '$lib/components/Menu.svelte';
	import SettingsPanel from '$lib/components/SettingsPanel.svelte';
	import { popupNative } from '$lib/nativeMenu';
	import { applyTheme } from '$lib/theme';

	// The native side swaps the settings pane for the account view in one frame,
	// so the pane gets this long to fade before the swap is requested.
	const PANE_OUT = 120;

	const alt = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform) ? '⌥' : 'Alt ';

	type Notice = {
		id: string;
		kind: 'error' | 'progress' | 'done' | 'info';
		text: string;
		action?: { label: string; run: () => void };
	};

	let snapshot = $state<Snapshot | null>(null);
	let surfaces = $state<Surface[]>([]);
	let ready = $state(false);
	let settings = $state(false);
	let editing = $state<string | null>(null);
	let adding = $state(false);
	let creating = $state(false);
	let label = $state('');
	let notices = $state<Notice[]>([]);
	let request = $state<MenuRequest | null>(null);
	// Views with a page load in flight, as "account/suite".
	let loading = $state(new Set<string>());
	// What the user just clicked, shown as selected while the native switch is in flight.
	let pending = $state.raw<{ id: string; app?: string } | null>(null);

	const timers = new Map<string, ReturnType<typeof setTimeout>>();

	const accounts = $derived(snapshot?.accounts ?? []);
	const empty = $derived(snapshot !== null && snapshot.accounts.length === 0);
	const compact = $derived(snapshot?.railCollapsed ?? false);
	const activeId = $derived(pending?.id ?? snapshot?.activeId ?? null);
	const suite = $derived.by(() => {
		const hidden = new Set(snapshot?.hiddenApps ?? []);
		return (snapshot?.suite ?? []).filter((app) => !hidden.has(app.id));
	});
	const open = $derived.by(() => {
		const byAccount = new Map<string, Set<string>>();
		for (const { accountId, suiteId } of surfaces) {
			const set = byAccount.get(accountId) ?? new Set<string>();
			set.add(suiteId);
			byAccount.set(accountId, set);
		}
		return byAccount;
	});
	const loadingBy = $derived.by(() => {
		const byAccount = new Map<string, Set<string>>();
		for (const key of loading) {
			const [accountId, suiteId] = key.split('/');
			const set = byAccount.get(accountId) ?? new Set<string>();
			set.add(suiteId);
			byAccount.set(accountId, set);
		}
		return byAccount;
	});
	const none = new Set<string>();
	const icons = $derived(Object.fromEntries((snapshot?.suite ?? []).map((app) => [app.id, app.icon])));

	const kinds: Array<[string, string]> = [
		['docs', 'Document'],
		['sheets', 'Spreadsheet'],
		['slides', 'Presentation'],
		['forms', 'Form']
	];

	function frontOf(id: string) {
		if (pending?.id === id && pending.app) return pending.app;
		return snapshot?.fronts[id] ?? 'mail';
	}

	function accept(next: Snapshot) {
		snapshot = next;
		applyTheme(next.theme);
	}

	function notify(notice: Notice, ttl = 0) {
		clearTimeout(timers.get(notice.id));
		const at = notices.findIndex((item) => item.id === notice.id);
		if (at >= 0) notices[at] = notice;
		else notices = [...notices.slice(-2), notice];
		if (ttl) timers.set(notice.id, setTimeout(() => dismiss(notice.id), ttl));
	}

	function dismiss(id: string) {
		clearTimeout(timers.get(id));
		timers.delete(id);
		notices = notices.filter((item) => item.id !== id);
	}

	function fail(err: unknown) {
		const text = err instanceof Error ? err.message : String(err);
		notify({ id: `error:${text}`, kind: 'error', text }, 6000);
	}

	async function run(action: () => Promise<Snapshot | void>) {
		try {
			const next = await action();
			if (next) accept(next);
			return true;
		} catch (err) {
			fail(err);
			return false;
		}
	}

	async function refreshSurfaces() {
		surfaces = await listSurfaces().catch(() => surfaces);
	}

	async function show(id: string, app?: string) {
		settings = false;
		if (editing !== id) editing = null;
		const mine = (pending = { id, app });
		await run(() => (app ? showSurface(id, app) : focusAccount(id)));
		if (pending === mine) pending = null;
	}

	async function cycle(delta: number) {
		settings = false;
		editing = null;
		await run(() => cycleAccount(delta));
	}

	async function openSettings() {
		editing = null;
		adding = false;
		try {
			await showSettings();
			settings = true;
		} catch (err) {
			fail(err);
		}
	}

	async function closeSettings() {
		if (!settings) return;
		settings = false;
		await new Promise((done) => setTimeout(done, PANE_OUT));
		const id = snapshot?.activeId;
		if (settings || !id) return;
		await run(() => showSurface(id, frontOf(id)));
	}

	async function setCompact(next: boolean) {
		editing = null;
		adding = false;
		await run(() => setRailCollapsed(next));
	}

	async function add() {
		const name = label.trim();
		if (name === '' || creating) return;
		creating = true;
		const created = await run(() => createAccount(name));
		creating = false;
		if (!created) return;
		label = '';
		adding = false;
		settings = false;
	}

	function startAdd() {
		if (compact) {
			// The name field needs room, so adding from the compact rail expands it first.
			void setCompact(false).then(() => (adding = true));
			return;
		}
		adding = true;
	}

	function cancelAdd() {
		adding = false;
		label = '';
	}

	async function editAccount(account: Account) {
		if (compact) await setCompact(false);
		await tick();
		editing = account.id;
	}

	function accountMenu(account: Account, index: number, event: MouseEvent) {
		openMenu({
			x: event.clientX,
			y: event.clientY,
			title: account.email ?? account.label,
			items: [
				{ label: 'Edit account…', icon: Pencil, action: () => void editAccount(account) },
				...(index > 0
					? [{ label: 'Move up', icon: ArrowUp, action: () => void run(() => moveAccount(account.id, -1)) }]
					: []),
				...(index < accounts.length - 1
					? [{ label: 'Move down', icon: ArrowDown, action: () => void run(() => moveAccount(account.id, 1)) }]
					: [])
			]
		});
	}

	function appMenu(account: Account, app: SuiteApp, event: MouseEvent) {
		const live = open.get(account.id)?.has(app.id) ?? false;
		openMenu({
			x: event.clientX,
			y: event.clientY,
			title: `${app.label} · ${account.label}`,
			items: [
				...(live
					? [{ label: 'Reload', icon: RotateCw, action: () => void run(() => reloadSurface(account.id, app.id)) }]
					: []),
				...(live && app.id !== 'mail'
					? [
							{
								label: 'Close',
								icon: X,
								action: () =>
									void run(async () => {
										const next = await closeSurface(account.id, app.id);
										await refreshSurfaces();
										return next;
									})
							}
						]
					: []),
				...(app.id !== 'mail'
					? [{ label: 'Hide from sidebar', icon: EyeOff, action: () => void run(() => setAppHidden(app.id, true)) }]
					: [])
			]
		});
	}

	function allApps(account: Account, event: MouseEvent) {
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		openMenu({
			x: box.right + 6,
			y: box.top,
			title: account.label,
			items: suite.map((app) => ({
				label: app.label,
				image: app.icon,
				current: frontOf(account.id) === app.id,
				action: () => void show(account.id, app.id)
			}))
		});
	}

	function newMenu(account: Account, event: MouseEvent) {
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		openMenu({
			x: compact ? box.right + 6 : box.left,
			y: compact ? box.top : box.bottom + 4,
			title: 'New',
			items: kinds.map(([kind, name]) => ({
				label: name,
				image: icons[kind],
				action: () => void run(() => newTab(account.id, kind))
			}))
		});
	}

	function tabMenu(account: Account, tab: Tab, event: MouseEvent) {
		const tabs = account.tabs ?? [];
		const index = tabs.findIndex((item) => item.id === tab.id);
		const live = open.get(account.id)?.has(tab.id) ?? false;
		openMenu({
			x: event.clientX,
			y: event.clientY,
			title: tab.title || 'Untitled',
			items: [
				...(live
					? [{ label: 'Reload', icon: RotateCw, action: () => void run(() => reloadSurface(account.id, tab.id)) }]
					: []),
				{
					label: 'Copy link',
					icon: Link,
					action: () =>
						void navigator.clipboard.writeText(tab.url).then(
							() => notify({ id: 'copied', kind: 'info', text: 'Link copied' }, 2500),
							fail
						)
				},
				...(index > 0
					? [{ label: 'Move up', icon: ArrowUp, action: () => void run(() => moveTab(account.id, tab.id, -1)) }]
					: []),
				...(index < tabs.length - 1
					? [{ label: 'Move down', icon: ArrowDown, action: () => void run(() => moveTab(account.id, tab.id, 1)) }]
					: []),
				{ label: 'Close tab', icon: X, action: () => void run(() => closeTab(account.id, tab.id)) }
			]
		});
	}

	function openMenu(next: MenuRequest) {
		if (next.items.length === 0) return;
		if (compact) {
			request = null;
			popupNative(next).catch(fail);
		} else {
			request = next;
		}
	}

	function onDownload(download: DownloadEvent) {
		const id = `download:${download.url}`;
		const name = download.name || 'file';
		if (download.status === 'started') {
			notify({ id, kind: 'progress', text: `Downloading ${name}` });
		} else if (download.status === 'done') {
			const path = download.path;
			notify(
				{
					id,
					kind: 'done',
					text: `Downloaded ${name}`,
					action: path ? { label: 'Show', run: () => void run(() => revealDownload(path)) } : undefined
				},
				8000
			);
		} else {
			notify({ id, kind: 'error', text: `${name} did not download` }, 8000);
		}
	}

	function focusOnMount(node: HTMLInputElement) {
		node.focus();
	}

	function onKey(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			if (request) request = null;
			else if (editing) editing = null;
			else if (adding) cancelAdd();
			else if (settings) void closeSettings();
			return;
		}
		if (event.ctrlKey && !event.altKey && event.key === ',') {
			event.preventDefault();
			void (settings ? closeSettings() : openSettings());
			return;
		}
		if (!snapshot || !event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		if (event.key >= '1' && event.key <= '9') {
			const account = snapshot.accounts[Number(event.key) - 1];
			if (!account) return;
			event.preventDefault();
			void show(account.id);
			return;
		}
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			void cycle(event.key === 'ArrowDown' ? 1 : -1);
		}
	}

	onMount(() => {
		if (!('__TAURI_INTERNALS__' in window)) {
			ready = true;
			fail('Run this with pnpm tauri dev. It is the desktop shell, not a web page.');
			return;
		}
		const stops: Array<() => void> = [];
		const on = <T,>(name: string, handle: (payload: T) => void) =>
			void listen<T>(name, (event) => handle(event.payload)).then((stop) => stops.push(stop));
		void refreshSurfaces();
		void reportCalls().catch(fail);
		void listAccounts()
			.then(accept)
			.catch(fail)
			.finally(() => (ready = true));
		on<Snapshot>('accounts-updated', (next) => {
			accept(next);
			void refreshSurfaces();
		});
		on<Theme>('theme-updated', (theme) => {
			applyTheme(theme);
			if (snapshot) snapshot = { ...snapshot, theme };
		});
		on<[string, string, boolean]>('surface-loading', ([accountId, suiteId, busy]) => {
			const key = `${accountId}/${suiteId}`;
			if (busy === loading.has(key)) return;
			const next = new Set(loading);
			if (busy) next.add(key);
			else next.delete(key);
			loading = next;
			if (busy) void refreshSurfaces();
		});
		on<[string, string]>('surface-closed', ([accountId, suiteId]) => {
			const key = `${accountId}/${suiteId}`;
			if (loading.has(key)) loading = new Set([...loading].filter((item) => item !== key));
			void refreshSurfaces();
		});
		on<DownloadEvent>('download', onDownload);
		on<null>('open-settings', () => void openSettings());
		return () => {
			timers.forEach((timer) => clearTimeout(timer));
			stops.forEach((stop) => stop());
		};
	});
</script>

<svelte:window
	onkeydown={onKey}
	oncontextmenu={(event) => {
		// The rail is an app, not a page. Text fields keep their menu for copy and paste.
		if (!(event.target instanceof HTMLInputElement)) event.preventDefault();
	}}
/>

<div class="frame" class:compact style:--rail={compact ? '68px' : '300px'}>
	<aside>
		<header>
			<span class="mark" title="Workspaced"><Layers size={14} strokeWidth={2} /></span>
			{#if !compact}
				<h1 in:fade={{ duration: 160 }}>Workspaced</h1>
			{/if}
		</header>

		{#if ready && snapshot}
			<div class="scroll" in:fade={{ duration: 160 }}>
				<ol>
					{#each accounts as account, index (account.id)}
						<li animate:flip={{ duration: 220, easing: cubicOut }} transition:slide={{ duration: 180 }}>
							<AccountRow
								{account}
								{index}
								{compact}
								asleep={snapshot.asleep.includes(account.id)}
								{suite}
								{alt}
								count={accounts.length}
								active={activeId === account.id}
								editing={editing === account.id}
								unread={snapshot.unread[account.id] ?? 0}
								front={frontOf(account.id)}
								open={open.get(account.id) ?? none}
								loading={loadingBy.get(account.id) ?? none}
								onpick={() => show(account.id)}
								onapp={(app) => show(account.id, app)}
								onappmenu={(app, event) => appMenu(account, app, event)}
								onmoreapps={(event) => allApps(account, event)}
								{icons}
								ontabclose={(tab) => run(() => closeTab(account.id, tab.id))}
								ontabmenu={(tab, event) => tabMenu(account, tab, event)}
								onnewtab={(event) => newMenu(account, event)}
								onmenu={(event) => accountMenu(account, index, event)}
								onedit={(on) => (editing = on ? account.id : null)}
								onrename={(next) => run(() => renameAccount(account.id, next))}
								oncolor={(color) => run(() => setAccountColor(account.id, color))}
								onmove={(delta) => run(() => moveAccount(account.id, delta))}
								onremove={() => {
									editing = null;
									void run(() => removeAccount(account.id));
								}}
							/>
						</li>
					{/each}
				</ol>

				{#if empty}
					{#if !compact}
						<p class="hint">Your accounts will show up here.</p>
					{/if}
				{:else if adding && !compact}
					<form
						class="add"
						in:fade={{ duration: 120 }}
						onsubmit={(event) => {
							event.preventDefault();
							void add();
						}}
					>
						<span class="plus"><Plus size={14} strokeWidth={2} /></span>
						<input
							bind:value={label}
							placeholder="Account name"
							maxlength="48"
							spellcheck="false"
							aria-label="New account name"
							disabled={creating}
							use:focusOnMount
							onblur={() => {
								if (label.trim() === '') cancelAdd();
							}}
						/>
					</form>
				{:else}
					<button class="add" title="Add account" aria-label="Add account" in:fade={{ duration: 120 }} onclick={startAdd}>
						<span class="plus"><Plus size={14} strokeWidth={2} /></span>
						{#if !compact}Add account{/if}
					</button>
				{/if}
			</div>
		{/if}

		<footer>
			{#if compact && notices.length}
				<!-- No room for text in the compact rail, so a dot stands in. Hover it to read the notices. -->
				<div
					class="notice-dot {notices.at(-1)?.kind}"
					role="status"
					title={notices.map((notice) => notice.text).join('\n')}
					transition:fade={{ duration: 150 }}
				></div>
			{/if}
			{#if !empty}
				<button
					class="ghost"
					class:on={settings}
					title="Settings (Ctrl ,)"
					aria-label="Settings"
					aria-pressed={settings}
					onclick={() => (settings ? closeSettings() : openSettings())}
				>
					<span class="spin"><Settings size={16} strokeWidth={1.75} /></span>
				</button>
			{/if}
			<button
				class="ghost"
				title={compact ? 'Expand sidebar' : 'Collapse sidebar'}
				aria-label={compact ? 'Expand sidebar' : 'Collapse sidebar'}
				onclick={() => setCompact(!compact)}
			>
				{#if compact}
					<PanelLeftOpen size={16} strokeWidth={1.75} />
				{:else}
					<PanelLeftClose size={16} strokeWidth={1.75} />
				{/if}
			</button>
		</footer>

		{#if notices.length && !compact}
			<div class="notices">
				{#each notices as notice (notice.id)}
					<div
						class="notice {notice.kind}"
						role={notice.kind === 'error' ? 'alert' : 'status'}
						animate:flip={{ duration: 180 }}
						transition:fly={{ y: 8, duration: 180 }}
					>
						<span class="glyph">
							{#if notice.kind === 'error'}
								<CircleAlert size={14} strokeWidth={2} />
							{:else if notice.kind === 'info'}
								<Check size={14} strokeWidth={2} />
							{:else if notice.kind === 'progress'}
								<span class="spinner"><LoaderCircle size={14} strokeWidth={2} /></span>
							{:else}
								<Download size={14} strokeWidth={2} />
							{/if}
						</span>
						<p>{notice.text}</p>
						{#if notice.action}
							{@const action = notice.action}
							<button
								class="link"
								onclick={() => {
									action.run();
									dismiss(notice.id);
								}}
							>
								{action.label}
							</button>
						{/if}
						<button class="ghost small" title="Dismiss" aria-label="Dismiss" onclick={() => dismiss(notice.id)}>
							<X size={13} strokeWidth={1.75} />
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</aside>

	<main>
		{#if empty}
			<div class="pane welcome" in:fly={{ y: 10, duration: 260, easing: cubicOut }}>
				<span class="mark big"><Layers size={22} strokeWidth={1.75} /></span>
				<h2>Add your first account</h2>
				<p>
					Each account keeps its own Google sign-in, so work, personal and client mail can all stay open side by
					side. Name it, then sign in once.
				</p>
				<form
					onsubmit={(event) => {
						event.preventDefault();
						void add();
					}}
				>
					<input
						bind:value={label}
						placeholder="Work, Personal, a client…"
						maxlength="48"
						spellcheck="false"
						aria-label="Account name"
						disabled={creating}
						use:focusOnMount
					/>
					<button class="primary" type="submit" disabled={creating || label.trim() === ''}>Add account</button>
				</form>
			</div>
		{:else if settings && snapshot}
			<div class="pane" in:fly={{ x: 12, duration: 220, easing: cubicOut }} out:fade={{ duration: PANE_OUT }}>
				<SettingsPanel
					{snapshot}
					{alt}
					onidentity={(identity) => run(() => setBrowserIdentity(identity))}
					oncompact={(next) => setCompact(next)}
					onapphidden={(suiteId, hidden) => run(() => setAppHidden(suiteId, hidden))}
					onsleep={(minutes) => run(() => setSleepAfter(minutes))}
					onpagetheme={(next) => run(() => setPageTheme(next))}
					onclose={closeSettings}
					onquit={() => quit()}
				/>
			</div>
		{/if}
	</main>
</div>

{#if request}
	<Menu {...request} onclose={() => (request = null)} />
{/if}

<style>
	/* The rail keeps its width whether or not the native side has widened this webview. */
	.frame {
		height: 100vh;
		display: grid;
		grid-template-columns: var(--rail) minmax(0, 1fr);
	}

	aside {
		position: relative;
		min-height: 0;
		display: flex;
		flex-direction: column;
		border-right: 1px solid var(--line);
	}

	header {
		height: 52px;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 12px 0 16px;
		flex: none;
	}

	.compact header {
		justify-content: center;
		padding: 0;
	}

	h1 {
		margin: 0;
		color: var(--fg-bright);
		font-size: 14px;
		font-weight: 650;
		letter-spacing: -0.01em;
	}

	.mark {
		width: 24px;
		height: 24px;
		display: grid;
		place-items: center;
		flex: none;
		border-radius: 7px;
		background: var(--accent);
		color: var(--on-accent);
	}

	.mark.big {
		width: 44px;
		height: 44px;
		border-radius: 12px;
	}

	.ghost {
		width: 30px;
		height: 30px;
		display: grid;
		place-items: center;
		flex: none;
		padding: 0;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: var(--fg-dim);
		transition:
			background var(--fast) var(--ease),
			color var(--fast) var(--ease);
	}

	.ghost.small {
		width: 24px;
		height: 24px;
		border-radius: 6px;
	}

	.spin {
		display: grid;
		transition: transform 320ms var(--ease);
	}

	.ghost.on .spin {
		transform: rotate(60deg);
	}

	.ghost:hover,
	.ghost.on {
		background: var(--hover);
		color: var(--fg);
	}

	.scroll {
		flex: 1;
		min-height: 0;
		overflow-x: hidden;
		overflow-y: auto;
		padding: 0 8px 12px;
	}

	.compact .scroll {
		padding: 0 10px 12px;
		scrollbar-width: none;
	}

	.compact .scroll::-webkit-scrollbar {
		display: none;
	}

	ol {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li {
		padding-bottom: 2px;
	}

	.hint {
		margin: 0;
		padding: 6px 8px;
		color: var(--fg-dim);
	}

	.add {
		width: 100%;
		height: 40px;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 8px;
		border: 0;
		border-radius: 10px;
		background: transparent;
		color: var(--fg-dim);
		text-align: left;
		transition:
			background var(--fast) var(--ease),
			color var(--fast) var(--ease);
	}

	.compact .add {
		width: 48px;
		height: 48px;
		justify-content: center;
		padding: 0;
	}

	.compact .plus {
		width: 38px;
		height: 38px;
		border-radius: 12px;
	}

	button.add:hover {
		background: var(--hover);
		color: var(--fg);
	}

	form.add {
		background: var(--hover);
	}

	.plus {
		width: 24px;
		height: 24px;
		display: grid;
		place-items: center;
		flex: none;
		border-radius: 7px;
		box-shadow: inset 0 0 0 1px var(--line);
		transition: border-radius var(--base) var(--ease);
	}

	.add input {
		flex: 1;
		min-width: 0;
		height: 100%;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--fg-bright);
	}

	.add input:focus {
		outline: none;
	}

	input::placeholder {
		color: color-mix(in srgb, var(--fg-dim) 70%, transparent);
	}

	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex: none;
		padding: 8px 10px 10px;
		border-top: 1px solid var(--line);
	}

	.compact footer {
		flex-direction: column;
		gap: 4px;
	}

	.notices {
		position: absolute;
		right: 8px;
		bottom: 56px;
		left: 8px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.notice {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 4px 6px 10px;
		border-radius: 10px;
		background: var(--bg-light);
		box-shadow:
			inset 0 0 0 1px var(--line),
			0 8px 24px rgb(0 0 0 / 0.3);
		color: var(--fg-bright);
		font-size: 12px;
	}

	.notice.error {
		background: color-mix(in srgb, var(--danger) 18%, var(--bg));
		box-shadow:
			inset 0 0 0 1px color-mix(in srgb, var(--danger) 40%, transparent),
			0 8px 24px rgb(0 0 0 / 0.3);
	}

	.glyph {
		display: grid;
		flex: none;
		color: var(--fg-dim);
	}

	.error .glyph {
		color: var(--danger);
	}

	.info .glyph,
	.done .glyph {
		color: var(--accent);
	}

	.spinner {
		display: grid;
		animation: spin 900ms linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	.notice p {
		flex: 1;
		min-width: 0;
		margin: 0;
		padding: 3px 0;
		line-height: 1.4;
		overflow-wrap: anywhere;
	}

	.link {
		flex: none;
		height: 24px;
		padding: 0 8px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--accent);
		font-size: 12px;
		font-weight: 600;
		transition: background var(--fast) var(--ease);
	}

	.link:hover {
		background: var(--hover);
	}

	.notice-dot {
		width: 8px;
		height: 8px;
		margin: 4px 0 8px;
		border-radius: 50%;
		background: var(--accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
	}

	.notice-dot.error {
		background: var(--danger);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--danger) 25%, transparent);
	}

	.notice-dot.progress {
		animation: pulse 1.2s var(--ease) infinite alternate;
	}

	@keyframes pulse {
		to {
			opacity: 0.35;
		}
	}

	main {
		min-width: 0;
		display: grid;
		overflow-y: auto;
		background: var(--bg-dark);
	}

	/* Enter and leave share one cell, so swapping panes never stacks them. */
	.pane {
		grid-area: 1 / 1;
		padding: 56px 64px;
	}

	.welcome {
		max-width: 560px;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		align-self: center;
		justify-self: center;
		gap: 14px;
	}

	h2 {
		margin: 10px 0 0;
		color: var(--fg-bright);
		font-size: 24px;
		font-weight: 650;
		letter-spacing: -0.02em;
	}

	.welcome p {
		margin: 0;
		color: var(--fg-dim);
		font-size: 14px;
		line-height: 1.55;
	}

	.welcome form {
		width: 100%;
		display: flex;
		gap: 8px;
		margin-top: 10px;
	}

	.welcome input {
		flex: 1;
		min-width: 0;
		height: 38px;
		padding: 0 12px;
		border: 1px solid var(--line);
		border-radius: 9px;
		background: var(--bg-darker);
		transition: border-color var(--fast) var(--ease);
	}

	.welcome input:focus {
		outline: none;
		border-color: var(--accent);
	}

	.primary {
		height: 38px;
		padding: 0 16px;
		border: 0;
		border-radius: 9px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 600;
		transition:
			opacity var(--fast) var(--ease),
			filter var(--fast) var(--ease);
	}

	.primary:not(:disabled):hover {
		filter: brightness(1.08);
	}

	.primary:disabled {
		opacity: 0.4;
	}
</style>
