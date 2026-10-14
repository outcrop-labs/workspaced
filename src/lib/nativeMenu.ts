import { LogicalPosition } from '@tauri-apps/api/dpi';
import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from '@tauri-apps/api/menu';
import type { MenuRequest } from '$lib/components/Menu.svelte';

/**
 * Shows a menu as a native popup. The compact sidebar is only a few dozen
 * pixels wide and a menu drawn inside it would be clipped by the Google view
 * beside it, so it uses this instead of the in-page menu.
 */
export async function popupNative(request: MenuRequest) {
	const items = await Promise.all(
		request.items.map((item) =>
			item.current !== undefined
				? CheckMenuItem.new({ text: item.label, checked: item.current, action: item.action })
				: MenuItem.new({ text: item.label, action: item.action })
		)
	);
	const head = request.title
		? [await MenuItem.new({ text: request.title, enabled: false }), await PredefinedMenuItem.new({ item: 'Separator' })]
		: [];
	const menu = await Menu.new({ items: [...head, ...items] });
	await menu.popup(new LogicalPosition(request.x, request.y));
}
