import { describe, expect, it } from 'vitest';

import type { BoardGeometry, SpaceId } from '../geometry';
import type { GameHistory, HistoryAction, PlayRecord } from '../actions';
import { HistoryCursor } from '../history-cursor';

function stubGeometry(): BoardGeometry {
	return {
		bounds: [-1, -1, 1, 1],
		spaces: [],
		slides: [],
		players: [
			{ player: 0, start_area: 60, start_exit: 0, home: 84 },
			{ player: 1, start_area: 61, start_exit: 15, home: 85 }
		]
	};
}

function advance(pawn: number, to: SpaceId, card = 'One'): PlayRecord {
	return {
		type: 'Play',
		card,
		mv: { type: 'Advance', pawn, card_value: 1, to },
		bumps: [],
		slides: []
	};
}

function simpleHistory(): GameHistory {
	return {
		seed: 1,
		num_players: 2,
		strategy_names: ['Random', 'Random'],
		rules_name: 'Standard',
		initial_deck_order: Array(45).fill('One'),
		starting_player: 0,
		turns: [
			{
				player: 0,
				actions: [
					{ type: 'Draw', card: 'One' } as HistoryAction,
					advance(0, 5) as HistoryAction
				]
			},
			{
				player: 1,
				actions: [
					{ type: 'Draw', card: 'One' } as HistoryAction,
					advance(0, 17) as HistoryAction
				]
			}
		],
		winners: [],
		truncated: false
	};
}

/** History with a 2-card extra-turn chain: turn 0 contains two Plays. */
function extraTurnHistory(): GameHistory {
	return {
		seed: 1,
		num_players: 2,
		strategy_names: ['Random', 'Random'],
		rules_name: 'Standard',
		initial_deck_order: Array(45).fill('One'),
		starting_player: 0,
		turns: [
			{
				player: 0,
				actions: [
					{ type: 'Draw', card: 'Two' },
					advance(0, 2, 'Two'),
					{ type: 'ExtraTurnGranted' },
					{ type: 'Draw', card: 'Three' },
					advance(0, 5, 'Three')
				] as HistoryAction[]
			},
			{
				player: 1,
				actions: [{ type: 'Draw', card: 'One' }, advance(0, 17)] as HistoryAction[]
			}
		],
		winners: [],
		truncated: false
	};
}

describe('HistoryCursor — shape', () => {
	it('indexes every Play in the flat plays list', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		expect(cursor.length).toBe(2);
		expect(cursor.index).toBe(0);
		expect(cursor.isAtStart).toBe(true);
		expect(cursor.isAtEnd).toBe(false);
	});

	it('flattens multi-Play turns (extra-turn chain)', () => {
		const cursor = new HistoryCursor(extraTurnHistory(), stubGeometry());
		expect(cursor.length).toBe(3);
	});
});

describe('HistoryCursor — stepForward', () => {
	it('returns beat with prev/next/player/record and advances cursor', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		const beat = cursor.stepForward();
		expect(beat).not.toBeNull();
		expect(beat!.player).toBe(0);
		expect(beat!.record.mv).toMatchObject({ type: 'Advance', to: 5 });
		expect(beat!.prev.pawn_positions[0][0]).toBe(60); // start area
		expect(beat!.next.pawn_positions[0][0]).toBe(5);
		expect(cursor.index).toBe(1);
	});

	it('returns null once past the last play', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		cursor.stepForward();
		cursor.stepForward();
		expect(cursor.isAtEnd).toBe(true);
		expect(cursor.stepForward()).toBeNull();
	});

	it('stamps current_player + turn_count at turn boundaries', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		const beat0 = cursor.stepForward()!;
		expect(beat0.next.current_player).toBe(0);
		expect(beat0.next.turn_count).toBe(0);
		const beat1 = cursor.stepForward()!;
		expect(beat1.next.current_player).toBe(1);
		expect(beat1.next.turn_count).toBe(1);
	});
});

describe('HistoryCursor — stepBack and jumpTo', () => {
	it('stepBack returns to the previous state', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		cursor.stepForward();
		cursor.stepForward();
		const rewound = cursor.stepBack();
		expect(rewound).not.toBeNull();
		expect(cursor.index).toBe(1);
		expect(rewound!.pawn_positions[0][0]).toBe(5); // P0 has played, P1 not yet
		expect(rewound!.pawn_positions[1][0]).toBe(61);
	});

	it('stepBack at start returns null', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		expect(cursor.stepBack()).toBeNull();
	});

	it('jumpTo(n) produces the same state as n stepForwards', () => {
		const g = stubGeometry();
		const h = extraTurnHistory();
		const a = new HistoryCursor(h, g);
		const b = new HistoryCursor(h, g);
		a.stepForward();
		a.stepForward();
		const sequential = a.currentState;
		const jumped = b.jumpTo(2);
		expect(jumped.pawn_positions).toEqual(sequential.pawn_positions);
		expect(jumped.deck_remaining).toEqual(sequential.deck_remaining);
		expect(jumped.discard).toEqual(sequential.discard);
	});

	it('jumpTo(length) reaches the final state', () => {
		const cursor = new HistoryCursor(extraTurnHistory(), stubGeometry());
		const end = cursor.jumpTo(cursor.length);
		expect(cursor.isAtEnd).toBe(true);
		expect(end.pawn_positions[0][0]).toBe(5); // P0 ended at 5 after extra-turn chain
		expect(end.pawn_positions[1][0]).toBe(17); // P1 ended at 17
	});

	it('jumpTo clamps out-of-range indices', () => {
		const cursor = new HistoryCursor(simpleHistory(), stubGeometry());
		cursor.jumpTo(-10);
		expect(cursor.index).toBe(0);
		cursor.jumpTo(1000);
		expect(cursor.index).toBe(2);
	});
});

describe('HistoryCursor — extra-turn chain folding', () => {
	it('applies intermediate ExtraTurnGranted + Draw between same-turn Plays', () => {
		const cursor = new HistoryCursor(extraTurnHistory(), stubGeometry());
		const first = cursor.stepForward()!;
		expect(first.next.pawn_positions[0][0]).toBe(2);
		expect(first.next.discard).toEqual(['Two']);
		// A Play clears drawn_card, so the snapshot between beats is always
		// drawn_card=null — the "what's about to be played" is carried on
		// beat.record.card instead.
		expect(first.next.drawn_card).toBeNull();
		const second = cursor.stepForward()!;
		expect(second.record.card).toBe('Three');
		// The second Play's beat folds in the intervening Draw('Three'),
		// so the resulting state has pushed both cards to discard.
		expect(second.next.pawn_positions[0][0]).toBe(5);
		expect(second.next.discard).toEqual(['Two', 'Three']);
		// The player stays the same across an extra-turn chain.
		expect(second.player).toBe(0);
	});
});
