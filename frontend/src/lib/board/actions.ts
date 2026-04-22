// Mirror of the Rust `Move`, `SplitLeg`, `BumpEvent`, `SlideEvent`, and
// history `Action` types. Fields use serde-default externally-tagged
// enums, so each variant is `{ type: ..., ... }`.

import type { PlayerId, SpaceId } from './geometry';

export interface SplitLeg {
	pawn: number;
	steps: number;
	to: SpaceId;
}

export type Move =
	| { type: 'Advance'; pawn: number; card_value: number; to: SpaceId }
	| { type: 'Retreat'; pawn: number; card_value: number; to: SpaceId }
	| { type: 'StartPawn'; pawn: number; to: SpaceId }
	| { type: 'Sorry'; my_pawn: number; their_player: PlayerId; their_pawn: number; to: SpaceId }
	| { type: 'SwapEleven'; my_pawn: number; their_player: PlayerId; their_pawn: number }
	| { type: 'SplitSeven'; first: SplitLeg; second: SplitLeg }
	| { type: 'Pass' };

export interface BumpEvent {
	player: PlayerId;
	pawn: number;
	from: SpaceId;
	to: SpaceId;
}

export interface SlideEvent {
	player: PlayerId;
	pawn: number;
	from: SpaceId;
	to: SpaceId;
	path: SpaceId[];
}

export interface PlayRecord {
	type: 'Play';
	card: string;
	mv: Move;
	bumps: BumpEvent[];
	slides: SlideEvent[];
}

export type HistoryAction =
	| { type: 'Draw'; card: string }
	| { type: 'ChooseCard'; hand_index: number; card: string }
	| PlayRecord
	| { type: 'Reshuffle' }
	| { type: 'ExtraTurnGranted' };

export interface TurnRecord {
	player: PlayerId;
	actions: HistoryAction[];
}

export interface GameHistory {
	seed: number;
	num_players: number;
	strategy_names: string[];
	rules_name: string;
	initial_deck_order: string[];
	starting_player: PlayerId;
	turns: TurnRecord[];
	winners: PlayerId[];
	truncated: boolean;
}

function lastPlayInTurn(turn: TurnRecord): PlayRecord | null {
	for (let ai = turn.actions.length - 1; ai >= 0; ai--) {
		const action = turn.actions[ai];
		if (action.type === 'Play') return action;
	}
	return null;
}

/**
 * Most recent `Play` action across history AND the currently-assembling
 * turn (if any). `current_turn` lives outside `history.turns` during
 * extra-turn chains (e.g. after a 2-card), so history alone is not
 * sufficient when the very latest play granted an extra turn.
 */
export function findLastPlay(
	history: GameHistory,
	currentTurn?: TurnRecord | null
): { player: PlayerId; record: PlayRecord } | null {
	if (currentTurn) {
		const record = lastPlayInTurn(currentTurn);
		if (record) return { player: currentTurn.player, record };
	}
	for (let ti = history.turns.length - 1; ti >= 0; ti--) {
		const turn = history.turns[ti];
		const record = lastPlayInTurn(turn);
		if (record) return { player: turn.player, record };
	}
	return null;
}
