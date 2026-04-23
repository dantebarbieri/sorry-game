import { describe, expect, it } from 'vitest';

import type { BoardGeometry, SpaceId } from '../geometry';
import type { GameHistory, HistoryAction, PlayRecord } from '../actions';
import {
	applyHistoryAction,
	finalReplayState,
	initialReplayState,
	type ReplayState
} from '../replay';

// The reducer reads only `geometry.players[p].start_area` and `.home`,
// so tests use a minimal stub — no track layout, no slides. Keeping it
// synthetic avoids having to run wasm to get a real geometry.
function stubGeometry(numPlayers: number): BoardGeometry {
	const players = Array.from({ length: numPlayers }, (_, p) => ({
		player: p,
		// Start areas: space IDs 60..63 (following the real engine's order
		// — tracks then starts then safeties then homes). Not strictly
		// needed to match the engine for reducer logic to work; any
		// distinct ids do.
		start_area: 60 + p,
		start_exit: p * 15,
		home: 84 + p
	}));
	return {
		bounds: [-1, -1, 1, 1],
		spaces: [],
		slides: [],
		players
	};
}

function historyShell(opts: Partial<GameHistory> = {}): GameHistory {
	return {
		seed: 42,
		num_players: 4,
		strategy_names: ['Random', 'Random', 'Random', 'Random'],
		rules_name: 'Standard',
		initial_deck_order: Array(45).fill('One'),
		starting_player: 0,
		turns: [],
		winners: [],
		truncated: false,
		...opts
	};
}

function play(mvTo: SpaceId, pawn: number, card = 'One'): PlayRecord {
	return {
		type: 'Play',
		card,
		mv: { type: 'Advance', pawn, card_value: 1, to: mvTo },
		bumps: [],
		slides: []
	};
}

describe('initialReplayState', () => {
	it('seeds every pawn at its start area', () => {
		const g = stubGeometry(4);
		const s = initialReplayState(historyShell(), g);
		expect(s.pawn_positions).toHaveLength(4);
		for (let p = 0; p < 4; p++) {
			expect(s.pawn_positions[p]).toEqual([
				g.players[p].start_area,
				g.players[p].start_area,
				g.players[p].start_area,
				g.players[p].start_area
			]);
		}
	});

	it('matches deck / discard / winner invariants', () => {
		const g = stubGeometry(4);
		const s = initialReplayState(historyShell(), g);
		expect(s.deck_remaining).toBe(45);
		expect(s.discard).toEqual([]);
		expect(s.drawn_card).toBeNull();
		expect(s.winners).toEqual([]);
		expect(s.truncated).toBe(false);
		expect(s.current_player).toBe(0);
	});

	it('throws for a player index not covered by geometry', () => {
		const g = stubGeometry(2);
		expect(() => initialReplayState(historyShell({ num_players: 4 }), g)).toThrow();
	});
});

describe('applyHistoryAction — Draw / Reshuffle', () => {
	const g = stubGeometry(4);
	const base = initialReplayState(historyShell(), g);

	it('Draw decrements the deck and sets drawn_card', () => {
		const next = applyHistoryAction(base, 0, { type: 'Draw', card: 'Seven' }, g);
		expect(next.deck_remaining).toBe(44);
		expect(next.drawn_card).toBe('Seven');
		// Purity check — input state is untouched.
		expect(base.deck_remaining).toBe(45);
		expect(base.drawn_card).toBeNull();
	});

	it('Reshuffle empties discard into the deck count', () => {
		const mid: ReplayState = { ...base, deck_remaining: 0, discard: ['One', 'Two', 'Three'] };
		const next = applyHistoryAction(mid, 0, { type: 'Reshuffle' }, g);
		expect(next.deck_remaining).toBe(3);
		expect(next.discard).toEqual([]);
	});

	it('ExtraTurnGranted is a no-op on state', () => {
		const next = applyHistoryAction(base, 0, { type: 'ExtraTurnGranted' }, g);
		expect(next).toEqual(base);
	});
});

describe('applyHistoryAction — Play move types', () => {
	const g = stubGeometry(4);

	it('Advance moves the named pawn and records the card in discard', () => {
		const start: ReplayState = { ...initialReplayState(historyShell(), g), drawn_card: 'One' };
		const next = applyHistoryAction(start, 0, play(5, 1, 'One'), g);
		expect(next.pawn_positions[0][1]).toBe(5);
		expect(next.pawn_positions[0][0]).toBe(g.players[0].start_area); // untouched
		expect(next.discard).toEqual(['One']);
		expect(next.drawn_card).toBeNull();
	});

	it('Sorry moves the primary pawn and bumps the victim to their start', () => {
		const start = initialReplayState(historyShell(), g);
		// Place blue's pawn 2 on track space 10; we'll Sorry them off it.
		start.pawn_positions[1][2] = 10;
		const action: PlayRecord = {
			type: 'Play',
			card: 'Sorry',
			mv: { type: 'Sorry', my_pawn: 0, their_player: 1, their_pawn: 2, to: 10 },
			bumps: [{ player: 1, pawn: 2, from: 10, to: g.players[1].start_area }],
			slides: []
		};
		const next = applyHistoryAction(start, 0, action, g);
		expect(next.pawn_positions[0][0]).toBe(10);
		expect(next.pawn_positions[1][2]).toBe(g.players[1].start_area);
	});

	it('SwapEleven swaps positions of both pawns', () => {
		const start = initialReplayState(historyShell(), g);
		start.pawn_positions[0][1] = 5;
		start.pawn_positions[2][3] = 17;
		const action: PlayRecord = {
			type: 'Play',
			card: 'Eleven',
			mv: { type: 'SwapEleven', my_pawn: 1, their_player: 2, their_pawn: 3 },
			bumps: [],
			slides: []
		};
		const next = applyHistoryAction(start, 0, action, g);
		expect(next.pawn_positions[0][1]).toBe(17);
		expect(next.pawn_positions[2][3]).toBe(5);
	});

	it('SplitSeven moves both legs', () => {
		const start = initialReplayState(historyShell(), g);
		start.pawn_positions[0][0] = 10;
		start.pawn_positions[0][1] = 20;
		const action: PlayRecord = {
			type: 'Play',
			card: 'Seven',
			mv: {
				type: 'SplitSeven',
				first: { pawn: 0, steps: 3, to: 13 },
				second: { pawn: 1, steps: 4, to: 24 }
			},
			bumps: [],
			slides: []
		};
		const next = applyHistoryAction(start, 0, action, g);
		expect(next.pawn_positions[0][0]).toBe(13);
		expect(next.pawn_positions[0][1]).toBe(24);
	});

	it('slides override the primary landing for the sliding pawn', () => {
		const start = initialReplayState(historyShell(), g);
		start.pawn_positions[0][0] = 0;
		const action: PlayRecord = {
			type: 'Play',
			card: 'One',
			mv: { type: 'Advance', pawn: 0, card_value: 1, to: 1 },
			bumps: [],
			slides: [{ player: 0, pawn: 0, from: 1, to: 5, path: [2, 3, 4, 5] }]
		};
		const next = applyHistoryAction(start, 0, action, g);
		// Primary mv.to = 1, slide overrides to 5.
		expect(next.pawn_positions[0][0]).toBe(5);
	});

	it('records a win once all four pawns land home', () => {
		const start = initialReplayState(historyShell(), g);
		start.pawn_positions[0] = [g.players[0].home, g.players[0].home, g.players[0].home, 40];
		const action: PlayRecord = {
			type: 'Play',
			card: 'One',
			mv: { type: 'Advance', pawn: 3, card_value: 1, to: g.players[0].home },
			bumps: [],
			slides: []
		};
		const next = applyHistoryAction(start, 0, action, g);
		expect(next.winners).toEqual([0]);
	});
});

describe('finalReplayState', () => {
	it('walks a hand-built history to completion', () => {
		const g = stubGeometry(2);
		const history = historyShell({
			num_players: 2,
			turns: [
				{
					player: 0,
					actions: [
						{ type: 'Draw', card: 'One' } as HistoryAction,
						play(5, 0, 'One') as HistoryAction
					]
				},
				{
					player: 1,
					actions: [
						{ type: 'Draw', card: 'Two' } as HistoryAction,
						play(10, 0, 'Two') as HistoryAction,
						{ type: 'ExtraTurnGranted' } as HistoryAction,
						{ type: 'Draw', card: 'Three' } as HistoryAction,
						play(13, 0, 'Three') as HistoryAction
					]
				}
			]
		});
		const s = finalReplayState(history, g);
		expect(s.pawn_positions[0][0]).toBe(5);
		expect(s.pawn_positions[1][0]).toBe(13);
		expect(s.discard).toEqual(['One', 'Two', 'Three']);
		expect(s.deck_remaining).toBe(45 - 3);
	});
});
