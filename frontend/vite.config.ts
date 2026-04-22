import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		fs: {
			// Allow serving files from the workspace root (so the WASM pkg in
			// ../sorry-wasm or any future shared assets can be read).
			allow: ['..']
		}
	}
});
