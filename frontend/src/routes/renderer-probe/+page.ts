// The renderer probe is a WebGL-heavy page that loads the WASM engine from
// the site root. SSR would try to evaluate the runtime-only `/pkg/…` import;
// render client-side only.
export const ssr = false;
