import { getContext, setContext } from 'svelte';
import { LIGHT_SKIN, DARK_SKIN, type BoardSkin } from '$lib/board/skins';

const THEME_KEY = Symbol('sorry:theme');
const STORAGE_KEY = 'sorry:skin';

export type ThemeMode = 'auto' | 'light' | 'dark';

export interface ThemeStore {
	/** Currently-resolved skin (what the UI should render). */
	get skin(): BoardSkin;
	/** User's selected mode: 'auto' follows `prefers-color-scheme`. */
	get mode(): ThemeMode;
	setMode(mode: ThemeMode): void;
}

function isMode(value: unknown): value is ThemeMode {
	return value === 'auto' || value === 'light' || value === 'dark';
}

/** Read the persisted mode. Missing / unrecognized values fall back to auto. */
function loadStoredMode(): ThemeMode {
	if (typeof localStorage === 'undefined') return 'auto';
	const raw = localStorage.getItem(STORAGE_KEY);
	return isMode(raw) ? raw : 'auto';
}

/** System preference at call time. Defaults to light when matchMedia is absent. */
function systemPrefersDark(): boolean {
	if (typeof window === 'undefined' || !window.matchMedia) return false;
	return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

export function createThemeStore(): ThemeStore {
	let mode = $state<ThemeMode>('auto');
	let systemDark = $state(false);

	// On the client, hydrate from storage + system preference and subscribe
	// to OS theme flips so `auto` tracks live.
	if (typeof window !== 'undefined') {
		mode = loadStoredMode();
		systemDark = systemPrefersDark();
		if (window.matchMedia) {
			const mql = window.matchMedia('(prefers-color-scheme: dark)');
			const onChange = (e: MediaQueryListEvent) => {
				systemDark = e.matches;
			};
			// addEventListener is the modern API; older Safari exposes
			// addListener. Feature-detect without crashing on either.
			if (mql.addEventListener) mql.addEventListener('change', onChange);
			else if ((mql as unknown as { addListener?: (fn: typeof onChange) => void }).addListener) {
				(mql as unknown as { addListener: (fn: typeof onChange) => void }).addListener(onChange);
			}
		}
	}

	const skin = $derived.by<BoardSkin>(() => {
		if (mode === 'light') return LIGHT_SKIN;
		if (mode === 'dark') return DARK_SKIN;
		return systemDark ? DARK_SKIN : LIGHT_SKIN;
	});

	return {
		get skin() {
			return skin;
		},
		get mode() {
			return mode;
		},
		setMode(next: ThemeMode) {
			mode = next;
			if (typeof localStorage === 'undefined') return;
			if (next === 'auto') localStorage.removeItem(STORAGE_KEY);
			else localStorage.setItem(STORAGE_KEY, next);
		}
	};
}

export function provideTheme(store: ThemeStore): ThemeStore {
	setContext(THEME_KEY, store);
	return store;
}

export function useTheme(): ThemeStore {
	const store = getContext<ThemeStore | undefined>(THEME_KEY);
	if (!store) throw new Error('useTheme() called outside a ThemeProvider');
	return store;
}
