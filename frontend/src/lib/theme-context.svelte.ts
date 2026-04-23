import { getContext, setContext } from 'svelte';
import { LIGHT_SKIN, DARK_SKIN, type BoardSkin } from '$lib/board/skins';

const THEME_KEY = Symbol('sorry:theme');
const STORAGE_KEY = 'sorry:skin';

export interface ThemeStore {
	get skin(): BoardSkin;
	set skin(value: BoardSkin);
	toggle(): void;
}

export function createThemeStore(initial: BoardSkin = LIGHT_SKIN): ThemeStore {
	let current = $state(initial);
	const store: ThemeStore = {
		get skin() {
			return current;
		},
		set skin(value: BoardSkin) {
			current = value;
			if (typeof localStorage !== 'undefined') {
				localStorage.setItem(STORAGE_KEY, value.id);
			}
		},
		toggle() {
			store.skin = current.id === 'light' ? DARK_SKIN : LIGHT_SKIN;
		}
	};
	return store;
}

export function loadStoredSkin(): BoardSkin {
	if (typeof localStorage === 'undefined') return LIGHT_SKIN;
	return localStorage.getItem(STORAGE_KEY) === 'dark' ? DARK_SKIN : LIGHT_SKIN;
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
