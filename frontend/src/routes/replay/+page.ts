// Replay loads WASM at runtime; skip SSR so the server never tries to
// evaluate the client-only `$lib/pkg/…` glue.
export const ssr = false;
