// Worker-scope WASM loader. Web Workers get their own JS + WASM instances
// — they cannot share the main-thread WASM memory — so this mirrors
// `src/lib/wasm.ts` but is imported only from `src/lib/worker.ts`.

interface WasmApi {
	default: (module_or_path?: string | URL | Request) => Promise<unknown>;
	simulate_one_with_history: (config_json: string) => string;
	get_available_strategies: () => string;
	get_available_rules: () => string;
}

let cached: WasmApi | null = null;

export async function loadWorkerWasm(): Promise<WasmApi> {
	if (cached) return cached;
	// Relative path — the worker entry lives at src/lib/worker.ts, so the
	// generated glue at src/lib/pkg/sorry_wasm.js is a sibling directory.
	const mod = (await import('./pkg/sorry_wasm.js')) as unknown as WasmApi;
	await mod.default();
	cached = mod;
	return mod;
}

export function parseJsonOrThrow<T>(raw: string): T {
	const parsed = JSON.parse(raw) as T | { error: string };
	if (parsed && typeof parsed === 'object' && 'error' in parsed) {
		throw new Error((parsed as { error: string }).error);
	}
	return parsed as T;
}
