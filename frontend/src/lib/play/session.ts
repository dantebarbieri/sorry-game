import type { PlaySetup } from './types';

const SETUP_KEY = 'sorry:last-setup';

export function loadSetup(): PlaySetup | null {
	if (typeof sessionStorage === 'undefined') return null;
	const raw = sessionStorage.getItem(SETUP_KEY);
	if (!raw) return null;
	try {
		const parsed = JSON.parse(raw) as Partial<PlaySetup> & {
			numPlayers?: number;
		};
		if (!parsed || !Array.isArray(parsed.seats) || !parsed.rules) return null;
		// Pad to 4 seats so older stored setups (fewer-seat variants) still
		// load cleanly. A numeric `numPlayers` from the previous schema is
		// interpreted by marking the excess seats as Empty.
		const seats: PlaySetup['seats'] = Array.from({ length: 4 }, (_, i) => {
			if (typeof parsed.numPlayers === 'number' && i >= parsed.numPlayers) {
				return { type: 'Empty' } as const;
			}
			return (
				parsed.seats?.[i] ?? ({ type: 'Bot', strategy: 'Random' } as const)
			);
		});
		return { rules: parsed.rules, seats };
	} catch {
		return null;
	}
}

export function saveSetup(setup: PlaySetup) {
	if (typeof sessionStorage === 'undefined') return;
	sessionStorage.setItem(SETUP_KEY, JSON.stringify(setup));
}
