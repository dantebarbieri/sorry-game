// Runtime loader for the wasm-pack bundle under `src/lib/pkg/`. The pkg
// lives in the source tree (not in `static/`) so Vite can resolve the
// generated JS glue as an ES module. The hand-declared interface matches
// the exports in `sorry-wasm/src/lib.rs`.

interface WasmApi {
	default: (module_or_path?: string | URL | Request) => Promise<unknown>;
	get_board_geometry: (rules_name: string) => string;
	get_available_rules: () => string;
	get_available_strategies: () => string;
	get_rules_info: (rules_name: string) => string;
	simulate_one_with_history: (config_json: string) => string;
	create_interactive_game: (config_json: string) => string;
	get_game_state: (game_id: number, viewer: number) => string;
	apply_action: (game_id: number, action_json: string) => string;
	apply_bot_action: (game_id: number, strategy_name: string) => string;
	get_game_history: (game_id: number) => string;
	destroy_interactive_game: (game_id: number) => string;
}

let cached: WasmApi | null = null;

export async function loadWasm(): Promise<WasmApi> {
	if (cached) return cached;
	const mod = (await import('$lib/pkg/sorry_wasm.js')) as unknown as WasmApi;
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
