// Pure SPA — everything renders client-side. The WASM engine and board
// renderer both depend on the browser at module scope, so SSR would break
// them. The fallback page from adapter-static boots the Svelte runtime and
// the client router takes over from there.
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'ignore';
