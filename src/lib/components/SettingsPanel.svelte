<script lang="ts">
	import Power from '@lucide/svelte/icons/power';
	import X from '@lucide/svelte/icons/x';
	import type { PageTheme, Snapshot } from '$lib/api';

	type Identity = Snapshot['browserIdentity'];

	type Props = {
		snapshot: Snapshot;
		/** How the platform writes the Alt modifier, ready to prefix a key. */
		alt: string;
		onidentity: (identity: Identity) => void;
		oncompact: (compact: boolean) => void;
		onapphidden: (suiteId: string, hidden: boolean) => void;
		onsleep: (minutes: number | null) => void;
		onpagetheme: (next: PageTheme) => void;
		onclose: () => void;
		onquit: () => void;
	};

	let { snapshot, alt, onidentity, oncompact, onapphidden, onsleep, onpagetheme, onclose, onquit }: Props = $props();

	const sleepChoices: Array<[number | null, string]> = [
		[5, 'After 5 min'],
		[30, 'After 30 min'],
		[null, 'Never']
	];
	const sleepIndex = $derived(Math.max(0, sleepChoices.findIndex(([minutes]) => minutes === snapshot.sleepAfter)));

	const hidden = $derived(new Set(snapshot.hiddenApps));

	const identities: Array<[Identity, string]> = [
		['chrome', 'Chrome'],
		['system', 'System']
	];
</script>

<div class="settings">
	<header>
		<h2>Settings</h2>
		<button class="close" title="Close" aria-label="Close settings" onclick={onclose}>
			<X size={16} strokeWidth={1.75} />
		</button>
	</header>

	<section>
		<h3>Sign-in identity</h3>
		<div class="segment" class:second={snapshot.browserIdentity === 'system'} role="radiogroup" aria-label="Sign-in identity">
			<span class="thumb"></span>
			{#each identities as [id, name] (id)}
				<button
					role="radio"
					aria-checked={snapshot.browserIdentity === id}
					onclick={() => {
						if (snapshot.browserIdentity !== id) onidentity(id);
					}}
				>
					{name}
				</button>
			{/each}
		</div>
		<p>
			How accounts introduce themselves to Google. Switch only if Google refuses to sign you in. Open apps
			reload when this changes, and your logins are kept.
		</p>
	</section>

	<section>
		<h3>Calls and Meet</h3>
		{#if snapshot.callsNative}
			<p>Meet and calls run in the app. Meeting links open as tabs under their account.</p>
		{:else if snapshot.callBrowser}
			<p>
				Meet links and calls open in <strong>{snapshot.callBrowser}</strong>, in a window of their own, because
				this system's webview has no WebRTC. Each account gets its own {snapshot.callBrowser} profile, so sign in
				there once per account. Once the webview gains WebRTC, calls move into the app by themselves.
			</p>
		{:else}
			<p>
				Meet links and calls open in your default browser, signed in as the account they came from, because
				this system's webview has no WebRTC. Once the webview gains WebRTC, calls move into the app by themselves.
			</p>
		{/if}
	</section>

	<section>
		<h3>Background accounts</h3>
		<div class="segment three" style:--at={sleepIndex} role="radiogroup" aria-label="Put unused accounts to sleep">
			<span class="thumb"></span>
			{#each sleepChoices as [minutes, name] (name)}
				<button
					role="radio"
					aria-checked={snapshot.sleepAfter === minutes}
					onclick={() => {
						if (snapshot.sleepAfter !== minutes) onsleep(minutes);
					}}
				>
					{name}
				</button>
			{/each}
		</div>
		<p>
			A Google account can take most of a gigabyte of memory. Accounts you have not used for a while go to sleep:
			their pages close, and a small check every two minutes keeps the unread count current and notifies you of new
			mail. Picking a sleeping account reloads it.
		</p>
	</section>

	<section>
		<h3>Sidebar</h3>
		<label class="toggle">
			<span>
				Compact sidebar
				<small>Show accounts as avatars only. Hover one for its name.</small>
			</span>
			<input
				type="checkbox"
				role="switch"
				checked={snapshot.railCollapsed}
				onchange={(event) => oncompact(event.currentTarget.checked)}
			/>
		</label>
	</section>

	<section>
		<h3>Apps</h3>
		<p>Choose which Google apps appear under each account. Mail is always there.</p>
		<div class="apps">
			{#each snapshot.suite as app (app.id)}
				{@const shown = !hidden.has(app.id)}
				<button
					class="app"
					class:shown
					role="checkbox"
					aria-checked={shown}
					disabled={app.id === 'mail'}
					onclick={() => onapphidden(app.id, shown)}
				>
					<img src={app.icon} alt="" draggable="false" />
					{app.label}
				</button>
			{/each}
		</div>
	</section>

	<section>
		<h3>Shortcuts</h3>
		<dl>
			<div>
				<dt><kbd>{alt}1</kbd> – <kbd>{alt}9</kbd></dt>
				<dd>Jump to an account</dd>
			</div>
			<div>
				<dt><kbd>{alt}↓</kbd> <kbd>{alt}↑</kbd></dt>
				<dd>Next or previous account</dd>
			</div>
			<div>
				<dt><kbd>Ctrl R</kbd></dt>
				<dd>Reload the page</dd>
			</div>
			<div>
				<dt><kbd>{alt}←</kbd> <kbd>{alt}→</kbd></dt>
				<dd>Back or forward</dd>
			</div>
			<div>
				<dt><kbd>Ctrl ,</kbd></dt>
				<dd>Open settings</dd>
			</div>
			<div>
				<dt><kbd>Esc</kbd></dt>
				<dd>Close settings</dd>
			</div>
		</dl>
	</section>

	<section>
		<h3>Appearance</h3>
		<p>Colors and font follow your desktop theme, currently <strong>{snapshot.theme.name}</strong>.</p>
		{#if snapshot.theme.omarchy}
			<label class="toggle">
				<span>
					Match Google apps to Omarchy
					<small>Gmail, Calendar, Drive, Chat, Docs and the rest take your theme's colors.</small>
				</span>
				<input
					type="checkbox"
					role="switch"
					checked={snapshot.pageTheme.pages}
					onchange={(event) => onpagetheme({ ...snapshot.pageTheme, pages: event.currentTarget.checked })}
				/>
			</label>
			<label class="toggle" class:off={!snapshot.pageTheme.pages}>
				<span>
					Theme document pages
					<small>Docs and Sheets pages too, not only the toolbars. Images in documents change color with them.</small>
				</span>
				<input
					type="checkbox"
					role="switch"
					disabled={!snapshot.pageTheme.pages}
					checked={snapshot.pageTheme.documents}
					onchange={(event) => onpagetheme({ ...snapshot.pageTheme, documents: event.currentTarget.checked })}
				/>
			</label>
		{/if}
	</section>

	<footer>
		<button class="quit" onclick={onquit}>
			<Power size={14} strokeWidth={1.75} />
			Quit Workspaced
		</button>
	</footer>
</div>

<style>
	.settings {
		width: 100%;
		max-width: 520px;
		display: flex;
		flex-direction: column;
		gap: 32px;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	h2,
	h3,
	p,
	dl,
	dd {
		margin: 0;
	}

	h2 {
		color: var(--fg-bright);
		font-size: 22px;
		font-weight: 650;
		letter-spacing: -0.02em;
	}

	h3 {
		color: var(--fg);
		font-size: 13px;
		font-weight: 600;
	}

	p {
		max-width: 46ch;
		color: var(--fg-dim);
		line-height: 1.5;
	}

	strong {
		color: var(--fg);
		font-weight: 550;
	}

	section {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.close,
	.quit {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: var(--fg-dim);
		transition:
			background var(--fast) var(--ease),
			color var(--fast) var(--ease);
	}

	.close {
		width: 30px;
		height: 30px;
		padding: 0;
	}

	.close:hover {
		background: var(--hover);
		color: var(--fg);
	}

	.segment {
		position: relative;
		width: 240px;
		display: grid;
		grid-template-columns: 1fr 1fr;
		padding: 3px;
		border-radius: 10px;
		background: var(--bg-dark);
		box-shadow: inset 0 0 0 1px var(--line);
	}

	.thumb {
		position: absolute;
		top: 3px;
		bottom: 3px;
		left: 3px;
		width: calc(50% - 3px);
		border-radius: 7px;
		background: var(--bg-light);
		box-shadow: 0 1px 2px rgb(0 0 0 / 0.25);
		transition: transform 220ms var(--ease);
	}

	.second .thumb {
		transform: translateX(100%);
	}

	.segment.three {
		width: 330px;
		grid-template-columns: repeat(3, 1fr);
	}

	.three .thumb {
		width: calc((100% - 6px) / 3);
		transform: translateX(calc(var(--at) * 100%));
	}

	.segment button {
		position: relative;
		height: 28px;
		border: 0;
		border-radius: 7px;
		background: transparent;
		color: var(--fg-dim);
		transition: color var(--base) var(--ease);
	}

	.segment button:hover,
	.segment button[aria-checked='true'] {
		color: var(--fg-bright);
	}

	dl {
		display: flex;
		flex-direction: column;
		border-radius: 10px;
		box-shadow: inset 0 0 0 1px var(--line);
	}

	dl > div {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 9px 12px;
	}

	dl > div + div {
		border-top: 1px solid var(--line);
	}

	dt {
		order: 2;
		color: var(--fg-dim);
		white-space: nowrap;
	}

	kbd {
		display: inline-block;
		min-width: 22px;
		padding: 1px 6px;
		border-radius: 5px;
		background: var(--hover);
		box-shadow: inset 0 0 0 1px var(--line);
		color: var(--fg);
		font: 11px/1.6 ui-monospace, monospace;
		text-align: center;
	}

	.toggle {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 12px;
		border-radius: 10px;
		box-shadow: inset 0 0 0 1px var(--line);
		cursor: pointer;
	}

	.toggle span {
		display: flex;
		flex-direction: column;
		gap: 2px;
		color: var(--fg);
	}

	.toggle.off {
		opacity: 0.5;
		cursor: default;
	}

	small {
		color: var(--fg-dim);
		font-size: 12px;
	}

	/* A switch drawn on the checkbox itself. */
	input[role='switch'] {
		position: relative;
		width: 34px;
		height: 20px;
		flex: none;
		margin: 0;
		border-radius: 10px;
		background: var(--muted);
		appearance: none;
		cursor: pointer;
		transition: background var(--base) var(--ease);
	}

	input[role='switch']::after {
		content: '';
		position: absolute;
		top: 2px;
		left: 2px;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--fg-bright);
		box-shadow: 0 1px 2px rgb(0 0 0 / 0.3);
		transition: transform 220ms var(--ease);
	}

	input[role='switch']:checked {
		background: var(--accent);
	}

	input[role='switch']:checked::after {
		transform: translateX(14px);
	}

	.apps {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
		gap: 6px;
	}

	.app {
		position: relative;
		height: 36px;
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 0 10px;
		border: 0;
		border-radius: 9px;
		background: transparent;
		box-shadow: inset 0 0 0 1px var(--line);
		color: var(--fg-dim);
		text-align: left;
		transition:
			background var(--fast) var(--ease),
			color var(--fast) var(--ease),
			box-shadow var(--fast) var(--ease);
	}

	.app img {
		width: 16px;
		height: 16px;
		opacity: 0.45;
		filter: grayscale(1);
		transition:
			opacity var(--fast) var(--ease),
			filter var(--fast) var(--ease);
	}

	.app:not(:disabled):hover {
		background: var(--hover);
	}

	.app.shown {
		background: var(--hover);
		color: var(--fg);
	}

	.app.shown img {
		opacity: 1;
		filter: none;
	}

	.app:disabled {
		cursor: default;
		opacity: 0.7;
	}

	footer {
		padding-top: 20px;
		border-top: 1px solid var(--line);
	}

	.quit {
		gap: 8px;
		height: 30px;
		margin-left: -10px;
		padding: 0 10px;
	}

	.quit:hover {
		background: color-mix(in srgb, var(--danger) 16%, transparent);
		color: var(--danger);
	}
</style>
