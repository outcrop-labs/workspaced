// Copies the FreeType and HarfBuzz that the build host's WebKitGTK was built
// against into src-tauri/appimage/lib, from where tauri.conf.json puts them
// into the AppImage. Runs as the beforeBundleCommand; does nothing off Linux.
//
// The AppImage carries the host's WebKitGTK, whose text rasteriser (Skia)
// is compiled against the host's FreeType headers, but linuxdeploy leaves
// FreeType and HarfBuzz out of the bundle on the assumption that any copy
// will do. FreeType's colour-font structs changed shape between releases, so
// a newer FreeType on the user's machine corrupts memory as soon as a page
// draws a COLRv1 emoji font, and the page's process dies.
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, realpathSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

if (process.platform !== 'linux') process.exit(0);

const libs = ['libfreetype.so.6', 'libharfbuzz.so.0'];
const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri', 'appimage', 'lib');
const arch = process.arch === 'x64' ? 'x86-64' : process.arch === 'arm64' ? 'AArch64' : '';
const fallbackDirs = [
	`/usr/lib/${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}-linux-gnu`,
	'/usr/lib64',
	'/usr/lib',
	'/lib64',
	'/lib'
];

let cache = '';
try {
	cache = execFileSync('ldconfig', ['-p'], { encoding: 'utf8' });
} catch {
	// Not every distribution has ldconfig on the path; the fallback dirs cover those.
}

function locate(lib) {
	const entries = cache
		.split('\n')
		.map((line) => line.trim())
		.filter((line) => line.startsWith(`${lib} `) && (!arch || line.includes(arch)))
		.map((line) => line.split(' => ')[1])
		.filter((path) => path && existsSync(path));
	if (entries.length) return entries[0];
	return fallbackDirs.map((dir) => join(dir, lib)).find((path) => existsSync(path));
}

mkdirSync(out, { recursive: true });
for (const lib of libs) {
	const found = locate(lib);
	if (!found) {
		console.error(`appimage-libs: ${lib} not found on this host; the AppImage needs the copy WebKitGTK was built with`);
		process.exit(1);
	}
	copyFileSync(realpathSync(found), join(out, lib));
	console.log(`appimage-libs: ${found} -> ${join(out, lib)}`);
}
