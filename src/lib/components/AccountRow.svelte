<script lang="ts">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import type { Account, SuiteApp, Tab } from '$lib/api';

	const palette: Array<[string, string]> = [
		['#e8a54b', 'Amber'],
		['#6ea8fe', 'Blue'],
		['#7dcea0', 'Green'],
		['#e07a5f', 'Clay'],
		['#c084fc', 'Violet'],
		['#5ec8d8', 'Cyan'],
		['#f0c14b', 'Gold'],
		['#f472b6', 'Pink'],
		['#a3e635', 'Lime'],
		['#fb7185', 'Rose']
	];

	type Props = {
		account: Account;
		index: number;
		count: number;
		active: boolean;
		editing: boolean;
		/** Avatar-only layout for the narrow sidebar. */
		compact: boolean;
		/** Views closed to save memory. A feed view still checks for mail. */
		asleep: boolean;
		unread: number;
		/** The suite app this account is showing. */
		front: string;
		/** Suite apps that already have a live view for this account. */
		open: Set<string>;
		/** Suite apps whose view is loading a page. */
		loading: Set<string>;
		/** The apps to offer, already filtered by the user's choice. */
		suite: SuiteApp[];
		/** Every suite app, for tab icons. */
		icons: Record<string, string>;
		/** How the platform writes the Alt modifier, ready to prefix a key. */
		alt: string;
		onpick: () => void;
		onapp: (suiteId: string) => void;
		onappmenu: (app: SuiteApp, event: MouseEvent) => void;
		/** Compact sidebar only: list every app, since only open ones are shown. */
		onmoreapps: (event: MouseEvent) => void;
		ontabclose: (tab: Tab) => void;
		ontabmenu: (tab: Tab, event: MouseEvent) => void;
		onnewtab: (event: MouseEvent) => void;
		onmenu: (event: MouseEvent) => void;
		onedit: (editing: boolean) => void;
		onrename: (label: string) => void;
		oncolor: (color: string) => void;
		onmove: (delta: number) => void;
		onremove: () => void;
	};

	let {
		account,
		index,
		count,
		active,
		editing,
		compact,
		asleep,
		unread,
		front,
		open,
		loading,
		suite,
		icons,
		alt,
		onpick,
		onapp,
		onappmenu,
		onmoreapps,
		ontabclose,
		ontabmenu,
		onnewtab,
		onmenu,
		onedit,
		onrename,
		oncolor,
		onmove,
		onremove
	}: Props = $props();

	let armed = $state(false);
	let disarm: ReturnType<typeof setTimeout> | undefined;

	const tabs = $derived(account.tabs ?? []);
	const titleOf = (tab: Tab) => tab.title || 'Untitled';

	const initial = $derived(Array.from(account.label.trim())[0]?.toUpperCase() ?? '?');
	const badge = $derived(unread > (compact ? 99 : 999) ? (compact ? '99+' : '999+') : String(unread));
	// The compact column lists only apps in use, or it would push every other account off screen.
	const shown = $derived(
		compact ? suite.filter((app) => app.id === front || open.has(app.id) || loading.has(app.id)) : suite
	);
	const subtitle = $derived(account.email && account.email !== account.label ? account.email : '');
	const tooltip = $derived(
		[account.label, subtitle, unread ? `${unread} unread` : '', asleep ? 'asleep, still checking mail' : '']
			.filter(Boolean)
			.join(' · ')
	);
	// Waking reloads Gmail, so the avatar spins until it is back.
	const waking = $derived(active && loading.has('mail'));

	$effect(() => {
		if (!editing) armed = false;
		return () => clearTimeout(disarm);
	});

	function focusSelect(node: HTMLInputElement) {
		node.focus();
		node.select();
	}

	function commit(input: HTMLInputElement) {
		const next = input.value.trim();
		if (next === '') {
			input.value = account.label;
			return;
		}
		if (next !== account.label) onrename(next);
	}

	function remove() {
		clearTimeout(disarm);
		if (armed) {
			armed = false;
			onremove();
			return;
		}
		armed = true;
		disarm = setTimeout(() => (armed = false), 3000);
	}

	function contextMenu(event: MouseEvent) {
		event.preventDefault();
		onmenu(event);
	}
</script>

{#snippet apps(column: boolean)}
	<div class="apps" class:column transition:slide={{ duration: 180 }}>
		{#each shown as app (app.id)}
			<button
				class="app"
				class:front={front === app.id}
				class:open={open.has(app.id)}
				class:loading={loading.has(app.id)}
				title={app.label}
				aria-label={app.label}
				aria-pressed={front === app.id}
				onclick={() => onapp(app.id)}
				oncontextmenu={(event) => {
					event.preventDefault();
					event.stopPropagation();
					onappmenu(app, event);
				}}
			>
				<img src={app.icon} alt="" draggable="false" />
			</button>
		{/each}
		{#if column && shown.length < suite.length}
			<button class="app more-apps" title="All apps" aria-label="All apps" onclick={(event) => onmoreapps(event)}>
				<LayoutGrid size={15} strokeWidth={1.75} />
			</button>
		{/if}
	</div>
{/snippet}

{#snippet tabList(column: boolean)}
	<div class="tabs" class:column transition:slide={{ duration: 180 }}>
		{#each tabs as tab (tab.id)}
			<div
				class="tab"
				class:front={front === tab.id}
				class:loading={loading.has(tab.id)}
				animate:flip={{ duration: 200, easing: cubicOut }}
				transition:slide={{ duration: 160 }}
			>
				<button
					class="open"
					title={titleOf(tab)}
					aria-label={titleOf(tab)}
					aria-pressed={front === tab.id}
					onclick={() => onapp(tab.id)}
					onauxclick={(event) => {
						if (event.button === 1) {
							event.preventDefault();
							ontabclose(tab);
						}
					}}
					oncontextmenu={(event) => {
						event.preventDefault();
						event.stopPropagation();
						ontabmenu(tab, event);
					}}
				>
					<span class="icon">
						{#if icons[tab.kind]}
							<img src={icons[tab.kind]} alt="" draggable="false" />
						{/if}
					</span>
					{#if !column}
						<span class="title">{titleOf(tab)}</span>
					{/if}
				</button>
				{#if !column}
					<button class="close" title="Close tab (Ctrl W)" aria-label="Close {titleOf(tab)}" onclick={() => ontabclose(tab)}>
						<X size={13} strokeWidth={2} />
					</button>
				{/if}
			</div>
		{/each}
		<button class="new" title="New document" aria-label="New document" onclick={(event) => onnewtab(event)}>
			<span class="icon"><Plus size={14} strokeWidth={2} /></span>
			{#if !column}<span class="title">New</span>{/if}
		</button>
	</div>
{/snippet}

{#if compact}
	<div class="account compact" class:active class:asleep class:waking style:--tint={account.color}>
		<button
			class="avatar-button"
			title={tooltip}
			aria-label={tooltip}
			aria-current={active ? 'true' : undefined}
			onclick={onpick}
			oncontextmenu={contextMenu}
		>
			<span class="avatar">{initial}</span>
			{#if unread > 0}
				<span class="badge dot" transition:slide={{ duration: 120, axis: 'x' }}>{badge}</span>
			{/if}
		</button>
		{#if active}
			{@render apps(true)}
			{@render tabList(true)}
		{/if}
	</div>
{:else}
	<div class="account" class:active class:editing class:asleep class:waking style:--tint={account.color}>
		<div class="row" role="presentation" oncontextmenu={contextMenu}>
			<button class="pick" title={tooltip} aria-current={active ? 'true' : undefined} onclick={onpick}>
				<span class="avatar">{initial}</span>
				<span class="text">
					<span class="name">{account.label}</span>
					{#if subtitle}
						<span class="email">{subtitle}</span>
					{/if}
				</span>
			</button>
			<div class="trail">
				{#if unread > 0}
					<span class="badge" aria-label="{unread} unread">{badge}</span>
				{:else if index < 9}
					<kbd>{alt}{index + 1}</kbd>
				{/if}
				<button
					class="more"
					title="Edit account"
					aria-label="Edit {account.label}"
					aria-expanded={editing}
					onclick={() => onedit(!editing)}
				>
					<Ellipsis size={16} strokeWidth={1.75} />
				</button>
			</div>
		</div>

		{#if editing}
			<div class="tray" transition:slide={{ duration: 180 }}>
				<input
					class="rename"
					value={account.label}
					maxlength="48"
					spellcheck="false"
					aria-label="Account name"
					use:focusSelect
					onblur={(event) => commit(event.currentTarget)}
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault();
							commit(event.currentTarget);
							onedit(false);
						}
						// Escape closes the tray from the window handler. Drop the draft first.
						if (event.key === 'Escape') event.currentTarget.value = account.label;
					}}
				/>
				<div class="swatches" role="radiogroup" aria-label="Account color">
					{#each palette as [color, name] (color)}
						<button
							class="swatch"
							class:picked={account.color === color}
							style:--swatch={color}
							role="radio"
							aria-checked={account.color === color}
							aria-label={name}
							title={name}
							onclick={() => oncolor(color)}
						></button>
					{/each}
				</div>
				<div class="tools">
					<button class="tool" title="Move up" aria-label="Move up" disabled={index === 0} onclick={() => onmove(-1)}>
						<ArrowUp size={14} strokeWidth={1.75} />
					</button>
					<button
						class="tool"
						title="Move down"
						aria-label="Move down"
						disabled={index === count - 1}
						onclick={() => onmove(1)}
					>
						<ArrowDown size={14} strokeWidth={1.75} />
					</button>
					<button class="remove" class:armed onclick={remove}>
						{armed ? 'Sign out and remove?' : 'Remove'}
					</button>
				</div>
			</div>
		{/if}

		{#if active}
			{@render apps(false)}
			{@render tabList(false)}
		{/if}
	</div>
{/if}

<style>
	.account {
		position: relative;
		border-radius: 10px;
		transition: background var(--base) var(--ease);
	}

	.account:not(.compact):hover,
	.account.editing {
		background: var(--hover);
	}

	.account:not(.compact).active {
		background: color-mix(in srgb, var(--tint) 13%, transparent);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 6px 4px 4px;
	}

	.pick {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 4px;
		border: 0;
		border-radius: 7px;
		background: transparent;
		text-align: left;
	}

	.avatar {
		width: 24px;
		height: 24px;
		display: grid;
		place-items: center;
		flex: none;
		border-radius: 7px;
		background: color-mix(in srgb, var(--tint) 22%, transparent);
		color: var(--tint);
		font-size: 12px;
		font-weight: 650;
		line-height: 1;
		transition:
			background var(--base) var(--ease),
			color var(--base) var(--ease),
			border-radius var(--base) var(--ease);
	}

	.active .avatar {
		background: var(--tint);
		color: var(--bg-darker);
	}

	/* Sleeping accounts fade back a little. They still get badges. */
	.avatar {
		position: relative;
	}

	.asleep:not(.active) .avatar {
		opacity: 0.55;
		filter: saturate(0.6);
	}

	.asleep:not(.active):hover .avatar {
		opacity: 0.85;
	}

	/* A ring turns around the avatar while a woken account loads. */
	.waking .avatar::after {
		content: '';
		position: absolute;
		inset: -3px;
		border: 1.5px solid transparent;
		border-top-color: var(--tint);
		border-radius: inherit;
		animation: spin 900ms linear infinite;
	}

	.text {
		min-width: 0;
		display: flex;
		flex-direction: column;
	}

	.name,
	.email {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.name {
		color: var(--fg-dim);
		transition: color var(--base) var(--ease);
	}

	.email {
		margin-top: -1px;
		color: color-mix(in srgb, var(--fg-dim) 75%, transparent);
		font-size: 11px;
	}

	.account:hover .name,
	.editing .name {
		color: var(--fg);
	}

	.active .name {
		color: var(--fg-bright);
		font-weight: 550;
	}

	/* The hint (shortcut or unread count) and the edit button share one slot and trade places on hover. */
	.trail {
		display: grid;
		place-items: center end;
		flex: none;
	}

	.trail > * {
		grid-area: 1 / 1;
		transition: opacity var(--fast) var(--ease);
	}

	kbd {
		padding: 0 4px;
		color: color-mix(in srgb, var(--fg-dim) 75%, transparent);
		font: 10px/1.6 ui-monospace, monospace;
		white-space: nowrap;
	}

	.badge {
		min-width: 20px;
		height: 18px;
		padding: 0 6px;
		border-radius: 9px;
		background: color-mix(in srgb, var(--tint) 24%, transparent);
		color: var(--fg-bright);
		font-size: 11px;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		line-height: 18px;
		text-align: center;
	}

	.active .badge {
		background: var(--tint);
		color: var(--bg-darker);
	}

	.more {
		width: 24px;
		height: 24px;
		display: grid;
		place-items: center;
		padding: 0;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--fg-dim);
		opacity: 0;
		transition:
			opacity var(--fast) var(--ease),
			background var(--fast) var(--ease),
			color var(--fast) var(--ease);
	}

	.more:hover,
	.editing .more {
		background: var(--press);
		color: var(--fg);
	}

	.account:hover .more,
	.account:focus-within .more,
	.editing .more {
		opacity: 1;
	}

	.account:hover .trail > :not(.more),
	.account:focus-within .trail > :not(.more),
	.editing .trail > :not(.more) {
		opacity: 0;
	}

	/* Compact sidebar */

	.compact {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 3px 0;
	}

	/* A pill on the sidebar edge marks the active account and grows on hover. */
	.compact::before {
		content: '';
		position: absolute;
		top: 15px;
		left: -10px;
		width: 4px;
		height: 8px;
		border-radius: 0 3px 3px 0;
		background: var(--fg-bright);
		opacity: 0;
		transform: scaleY(0.5);
		transition:
			opacity var(--base) var(--ease),
			transform var(--base) var(--ease),
			top var(--base) var(--ease),
			height var(--base) var(--ease);
	}

	.compact:hover::before {
		opacity: 0.6;
		transform: none;
	}

	.compact.active::before {
		top: 7px;
		height: 24px;
		opacity: 1;
		transform: none;
	}

	.avatar-button {
		position: relative;
		padding: 0;
		border: 0;
		background: transparent;
	}

	.compact .avatar {
		width: 38px;
		height: 38px;
		border-radius: 12px;
		font-size: 15px;
	}

	.compact .avatar-button:hover .avatar,
	.compact.active .avatar {
		border-radius: 10px;
	}

	.compact .avatar-button:hover .avatar {
		background: color-mix(in srgb, var(--tint) 34%, transparent);
	}

	.compact.active .avatar-button:hover .avatar {
		background: var(--tint);
	}

	.badge.dot {
		position: absolute;
		top: -5px;
		right: -7px;
		min-width: 18px;
		height: 16px;
		padding: 0 4px;
		border-radius: 8px;
		background: var(--danger);
		box-shadow: 0 0 0 2px var(--bg-darker);
		color: #fff;
		font-size: 10px;
		line-height: 16px;
	}

	/* Edit tray */

	.tray {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 4px 8px 10px;
	}

	.rename {
		width: 100%;
		height: 30px;
		padding: 0 9px;
		border: 1px solid var(--line);
		border-radius: 7px;
		background: color-mix(in srgb, var(--bg-darker) 60%, transparent);
		transition: border-color var(--fast) var(--ease);
	}

	.rename:focus {
		outline: none;
		border-color: var(--accent);
	}

	.swatches {
		display: flex;
		justify-content: space-between;
		padding: 0 2px;
	}

	.swatch {
		width: 16px;
		height: 16px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: var(--swatch);
		box-shadow: 0 0 0 0 transparent;
		transition:
			transform var(--fast) var(--ease),
			box-shadow var(--fast) var(--ease);
	}

	.swatch:hover {
		transform: scale(1.18);
	}

	.swatch.picked {
		box-shadow:
			0 0 0 2px var(--bg-darker),
			0 0 0 3.5px var(--swatch);
	}

	.tools {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.tool,
	.remove {
		height: 26px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--fg-dim);
		transition:
			background var(--fast) var(--ease),
			color var(--fast) var(--ease),
			opacity var(--fast) var(--ease);
	}

	.tool {
		width: 26px;
		padding: 0;
	}

	.tool:disabled {
		opacity: 0.3;
	}

	.tool:not(:disabled):hover {
		background: var(--press);
		color: var(--fg);
	}

	.remove {
		margin-left: auto;
		padding: 0 9px;
		font-size: 12px;
	}

	.remove:hover {
		background: color-mix(in srgb, var(--danger) 16%, transparent);
		color: var(--danger);
	}

	.remove.armed {
		background: var(--danger);
		color: #fff;
	}

	/* App grid */

	.apps {
		display: grid;
		grid-template-columns: repeat(8, 1fr);
		gap: 2px;
		padding: 2px 8px 8px;
	}

	.apps.column {
		grid-template-columns: 36px;
		justify-content: center;
		margin-top: 6px;
		padding: 6px 0 2px;
		border-top: 1px solid var(--line);
	}

	.app {
		position: relative;
		aspect-ratio: 1;
		display: grid;
		place-items: center;
		padding: 0;
		border: 0;
		border-radius: 8px;
		background: transparent;
		transition:
			background var(--fast) var(--ease),
			transform var(--fast) var(--ease);
	}

	.app img {
		width: 18px;
		height: 18px;
		display: block;
		opacity: 0.6;
		filter: saturate(0.7);
		transition:
			opacity var(--fast) var(--ease),
			filter var(--fast) var(--ease),
			transform var(--base) var(--ease);
	}

	.app:hover {
		background: var(--press);
	}

	.app:active {
		transform: scale(0.92);
	}

	.app:hover img,
	.app.open img,
	.app.front img {
		opacity: 1;
		filter: none;
	}

	.app.front {
		background: color-mix(in srgb, var(--fg) 14%, transparent);
	}

	.more-apps {
		color: var(--fg-dim);
	}

	.more-apps:hover {
		color: var(--fg);
	}

	/* A dot under apps that are loaded in the background. */
	.app::after {
		content: '';
		position: absolute;
		bottom: 2px;
		left: 50%;
		width: 3px;
		height: 3px;
		margin-left: -1.5px;
		border-radius: 50%;
		background: var(--fg-dim);
		opacity: 0;
		transform: scale(0);
		transition:
			opacity var(--base) var(--ease),
			transform var(--base) var(--ease);
	}

	.app.open:not(.front)::after {
		opacity: 1;
		transform: scale(1);
	}

	/* Tabs */

	.tabs {
		display: flex;
		flex-direction: column;
		gap: 1px;
		margin: 0 8px;
		padding: 6px 0 8px;
		border-top: 1px solid var(--line);
	}

	.tabs.column {
		align-items: center;
		margin: 6px 0 0;
	}

	.tab {
		position: relative;
		display: flex;
		align-items: center;
		border-radius: 7px;
		transition: background var(--fast) var(--ease);
	}

	.tab:hover {
		background: var(--press);
	}

	.tab.front {
		background: color-mix(in srgb, var(--fg) 14%, transparent);
	}

	.open,
	.new {
		min-width: 0;
		height: 30px;
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 0 6px;
		border: 0;
		border-radius: 7px;
		background: transparent;
		color: var(--fg-dim);
		text-align: left;
	}

	.open {
		flex: 1;
	}

	.column .open,
	.column .new {
		width: 36px;
		height: 36px;
		justify-content: center;
		padding: 0;
	}

	.tab.front .open,
	.tab:hover .open {
		color: var(--fg);
	}

	.icon {
		position: relative;
		width: 16px;
		height: 16px;
		display: grid;
		place-items: center;
		flex: none;
	}

	.icon img {
		width: 16px;
		height: 16px;
		transition: transform var(--base) var(--ease);
	}

	.tab.loading .icon img {
		transform: scale(0.7);
	}

	.tab.loading .icon::before {
		content: '';
		position: absolute;
		inset: -3px;
		border: 1.5px solid transparent;
		border-top-color: var(--accent);
		border-radius: 50%;
		animation: spin 800ms linear infinite;
	}

	.title {
		min-width: 0;
		overflow: hidden;
		font-size: 12.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.close {
		width: 22px;
		height: 22px;
		display: grid;
		place-items: center;
		flex: none;
		margin-right: 4px;
		padding: 0;
		border: 0;
		border-radius: 5px;
		background: transparent;
		color: var(--fg-dim);
		opacity: 0;
		transition:
			opacity var(--fast) var(--ease),
			background var(--fast) var(--ease);
	}

	.tab:hover .close,
	.tab:focus-within .close,
	.tab.front .close {
		opacity: 1;
	}

	.close:hover {
		background: var(--press);
		color: var(--fg);
	}

	.new {
		color: color-mix(in srgb, var(--fg-dim) 80%, transparent);
		transition:
			background var(--fast) var(--ease),
			color var(--fast) var(--ease);
	}

	.new:hover {
		background: var(--press);
		color: var(--fg);
	}

	/* A spinning arc around apps that are loading. */
	.app::before {
		content: '';
		position: absolute;
		inset: 3px;
		border: 1.5px solid transparent;
		border-top-color: var(--accent);
		border-radius: 50%;
		opacity: 0;
		transition: opacity var(--base) var(--ease);
	}

	.app.loading::before {
		opacity: 1;
		animation: spin 800ms linear infinite;
	}

	.app.loading img {
		transform: scale(0.82);
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}
</style>
