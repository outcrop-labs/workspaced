<script lang="ts" module>
	import type { Component } from 'svelte';

	export type MenuItem = {
		label: string;
		icon?: Component<{ size?: number; strokeWidth?: number }>;
		/** An image URL shown in place of an icon. */
		image?: string;
		/** Marks the item as the current choice. */
		current?: boolean;
		danger?: boolean;
		action: () => void;
	};

	export type MenuRequest = {
		x: number;
		y: number;
		title?: string;
		items: MenuItem[];
	};
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fade, scale } from 'svelte/transition';

	type Props = MenuRequest & { onclose: () => void };

	let { x, y, title, items, onclose }: Props = $props();

	let menu = $state<HTMLDivElement>();
	let left = $state(0);
	let top = $state(0);

	onMount(() => {
		if (!menu) return;
		// Keep the whole menu inside the window, flipping up or left near an edge.
		const box = menu.getBoundingClientRect();
		const pad = 6;
		left = x + box.width + pad > innerWidth ? Math.max(pad, x - box.width) : x;
		top = y + box.height + pad > innerHeight ? Math.max(pad, y - box.height) : y;
		menu.querySelector<HTMLButtonElement>('button')?.focus();
	});

	function pick(item: MenuItem) {
		onclose();
		item.action();
	}

	function onKey(event: KeyboardEvent) {
		if (!menu) return;
		const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button')];
		const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			onclose();
		} else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			const step = event.key === 'ArrowDown' ? 1 : -1;
			buttons[(at + step + buttons.length) % buttons.length]?.focus();
		} else if (event.key === 'Tab') {
			onclose();
		}
	}
</script>

<svelte:window
	onkeydowncapture={onKey}
	onpointerdown={(event) => {
		if (menu && !menu.contains(event.target as Node)) onclose();
	}}
	onblur={onclose}
	onresize={onclose}
/>

<div
	class="menu"
	role="menu"
	tabindex="-1"
	bind:this={menu}
	style:left="{left || x}px"
	style:top="{top || y}px"
	in:scale={{ start: 0.96, duration: 120, easing: cubicOut }}
	out:fade={{ duration: 90 }}
>
	{#if title}
		<p class="title">{title}</p>
	{/if}
	{#each items as item (item.label)}
		<button role="menuitem" class:danger={item.danger} class:current={item.current} onclick={() => pick(item)}>
			{#if item.image}
				<img src={item.image} alt="" draggable="false" />
			{:else if item.icon}
				{@const Icon = item.icon}
				<Icon size={14} strokeWidth={1.75} />
			{/if}
			{item.label}
		</button>
	{/each}
</div>

<style>
	.menu {
		position: fixed;
		z-index: 20;
		min-width: 176px;
		max-width: 240px;
		max-height: calc(100vh - 12px);
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		padding: 4px;
		border-radius: 10px;
		background: var(--bg-light);
		box-shadow:
			inset 0 0 0 1px var(--line),
			0 12px 32px rgb(0 0 0 / 0.35),
			0 2px 6px rgb(0 0 0 / 0.2);
		transform-origin: top left;
	}

	.title {
		margin: 0;
		padding: 6px 8px 4px;
		overflow: hidden;
		color: var(--fg-dim);
		font-size: 11px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	button {
		height: 30px;
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 0 8px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--fg);
		text-align: left;
		white-space: nowrap;
	}

	img {
		width: 14px;
		height: 14px;
		flex: none;
	}

	button.current {
		background: var(--hover);
		font-weight: 550;
	}

	button :global(svg) {
		flex: none;
		color: var(--fg-dim);
	}

	button:hover,
	button:focus-visible {
		outline: none;
		background: var(--press);
	}

	button.danger,
	button.danger :global(svg) {
		color: var(--danger);
	}

	button.danger:hover,
	button.danger:focus-visible {
		background: color-mix(in srgb, var(--danger) 16%, transparent);
	}
</style>
