import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// Pure-SPA output: every route becomes a bundle entry, and `fallback`
// is the HTML shell that nginx serves for any unknown path so client-side
// routing can take over. Matches the deploy model where `sorry.danteb.com`
// is reverse-proxied to an nginx container that also proxies `/api/*` to
// the Rust server.
/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			strict: true
		})
	}
};

export default config;
