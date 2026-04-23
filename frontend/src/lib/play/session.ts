import type { PlaySetup } from './types';

const SETUP_KEY = 'sorry:last-setup';

export function loadSetup(): PlaySetup | null {
	if (typeof sessionStorage === 'undefined') return null;
	const raw = sessionStorage.getItem(SETUP_KEY);
	if (!raw) return null;
	try {
		return JSON.parse(raw) as PlaySetup;
	} catch {
		return null;
	}
}

export function saveSetup(setup: PlaySetup) {
	if (typeof sessionStorage === 'undefined') return;
	sessionStorage.setItem(SETUP_KEY, JSON.stringify(setup));
}
