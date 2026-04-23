import { defineConfig } from 'vitest/config';

// Standalone vitest config — the pure-TS reducers under src/lib/board/
// don't touch Svelte/SvelteKit APIs, so tests run against plain Node
// without the sveltekit() plugin (which wants the runtime scaffolding).
export default defineConfig({
	test: {
		include: ['src/lib/**/__tests__/**/*.test.ts'],
		environment: 'node'
	}
});
