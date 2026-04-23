// Pure state reducer over `GameHistory`. Given `initial_deck_order`,
// `starting_player`, and `geometry.players`, this module reconstructs
// every intermediate `GameStateView` without calling the engine — the
// history is authoritative. Used by the replay cursor to step forward
// / back through a finished game at the UI layer.
//
// No legality checks: each `Action::Play` is trusted. We just apply
// positions via `mv` + `bumps` + `slides` and keep deck / discard / turn
// counts consistent.

import type { BoardGeometry, SpaceId } from './geometry';
import type { GameHistory, HistoryAction, PlayRecord, TurnRecord } from './actions';
import type { GameStateView } from './state';

const PAWNS_PER_PLAYER = 4;

export interface ReplayState extends GameStateView {}

/**
 * Initial state at `t=0`: every pawn on its start area, full deck, empty
 * discard, no card drawn yet, `current_player = history.starting_player`.
 */
export function initialReplayState(history: GameHistory, geometry: BoardGeometry): ReplayState {
	const pawn_positions: SpaceId[][] = [];
	for (let p = 0; p < history.num_players; p++) {
		const layout = geometry.players[p];
		if (!layout) throw new Error(`geometry missing PlayerLayout for player ${p}`);
		const row: SpaceId[] = [];
		for (let pw = 0; pw < PAWNS_PER_PLAYER; pw++) row.push(layout.start_area);
		pawn_positions.push(row);
	}
	return {
		num_players: history.num_players,
		pawn_positions,
		current_player: history.starting_player,
		turn_count: 0,
		drawn_card: null,
		deck_remaining: history.initial_deck_order.length,
		discard: [],
		winners: [],
		truncated: false,
		current_turn: null,
		action_needed: { type: 'GameOver', winners: [], truncated: false }
	};
}

function cloneState(state: ReplayState): ReplayState {
	return {
		...state,
		pawn_positions: state.pawn_positions.map((row) => row.slice()),
		discard: state.discard.slice(),
		winners: state.winners.slice(),
		current_turn: state.current_turn
			? { player: state.current_turn.player, actions: state.current_turn.actions.slice() }
			: null
	};
}

function applyPlay(state: ReplayState, player: number, play: PlayRecord): void {
	const mv = play.mv;
	switch (mv.type) {
		case 'Advance':
		case 'Retreat':
		case 'StartPawn':
			state.pawn_positions[player][mv.pawn] = mv.to;
			break;
		case 'Sorry':
			state.pawn_positions[player][mv.my_pawn] = mv.to;
			break;
		case 'SwapEleven': {
			const myPos = state.pawn_positions[player][mv.my_pawn];
			const theirPos = state.pawn_positions[mv.their_player][mv.their_pawn];
			state.pawn_positions[player][mv.my_pawn] = theirPos;
			state.pawn_positions[mv.their_player][mv.their_pawn] = myPos;
			break;
		}
		case 'SplitSeven':
			state.pawn_positions[player][mv.first.pawn] = mv.first.to;
			state.pawn_positions[player][mv.second.pawn] = mv.second.to;
			break;
		case 'Pass':
			break;
	}
	// Bumps land opponents at their start area. The recorded `to` is
	// authoritative — no need to ask geometry.
	for (const bump of play.bumps) {
		state.pawn_positions[bump.player][bump.pawn] = bump.to;
	}
	// Slides overwrite the primary-motion landing for the sliding pawn
	// and may bump others (already in `bumps`).
	for (const slide of play.slides) {
		state.pawn_positions[slide.player][slide.pawn] = slide.to;
	}
	state.drawn_card = null;
	state.discard.push(play.card);
}

function checkForWin(
	state: ReplayState,
	player: number,
	geometry: BoardGeometry
): void {
	if (state.winners.includes(player)) return;
	const homeSpace = geometry.players[player]?.home;
	if (homeSpace === undefined) return;
	const allHome = state.pawn_positions[player].every((s) => s === homeSpace);
	if (allHome) state.winners.push(player);
}

/**
 * Apply one `HistoryAction` (attributed to `player`) to `state` and
 * return the new state. Pure — does not mutate the input.
 */
export function applyHistoryAction(
	state: ReplayState,
	player: number,
	action: HistoryAction,
	geometry: BoardGeometry
): ReplayState {
	const next = cloneState(state);
	switch (action.type) {
		case 'Draw':
			next.deck_remaining = Math.max(0, next.deck_remaining - 1);
			next.drawn_card = action.card;
			break;
		case 'ChooseCard':
			// Hand-size-0 variants never emit this; higher variants would
			// affect the hand, which we don't model in ReplayState.
			break;
		case 'Play':
			applyPlay(next, player, action);
			checkForWin(next, player, geometry);
			break;
		case 'Reshuffle':
			next.deck_remaining += next.discard.length;
			next.discard = [];
			break;
		case 'ExtraTurnGranted':
			// HUD signal only — no state mutation. The next turn's `player`
			// staying the same is encoded directly in history.turns.
			break;
	}
	return next;
}

/**
 * Bump `state.current_player` and `turn_count` at the boundary between
 * two `TurnRecord`s. Called by the cursor after finishing one turn and
 * before starting the next.
 */
export function advanceToTurn(
	state: ReplayState,
	turn: TurnRecord,
	turnIndex: number
): ReplayState {
	const next = cloneState(state);
	next.current_player = turn.player;
	next.turn_count = turnIndex;
	next.current_turn = null;
	return next;
}

/**
 * Replay the full history to get the final state, useful for tests and
 * for jumping the cursor to the end in O(n).
 */
export function finalReplayState(history: GameHistory, geometry: BoardGeometry): ReplayState {
	let state = initialReplayState(history, geometry);
	for (let ti = 0; ti < history.turns.length; ti++) {
		const turn = history.turns[ti];
		state = advanceToTurn(state, turn, ti);
		for (const action of turn.actions) {
			state = applyHistoryAction(state, turn.player, action, geometry);
		}
	}
	state.winners = history.winners.slice();
	state.truncated = history.truncated;
	return state;
}
