// Replay cursor: walks a `GameHistory` at the granularity of `Play`
// actions (the only beats the renderer animates). Other actions — Draw,
// ChooseCard, Reshuffle, ExtraTurnGranted — fold silently into state
// between Plays. Back-stepping rebuilds from t=0; cheap enough for
// single-digit-thousand-turn games.

import type { BoardGeometry } from './geometry';
import type { GameHistory, PlayRecord } from './actions';
import {
	advanceToTurn,
	applyHistoryAction,
	initialReplayState,
	type ReplayState
} from './replay';

/** One animatable beat: a `Play` with the states it transitions between. */
export interface PlayBeat {
	index: number;
	player: number;
	record: PlayRecord;
	/** State BEFORE the play — renderer animates FROM this. */
	prev: ReplayState;
	/** State AFTER the play — renderer animates TO this. */
	next: ReplayState;
}

interface PlayLocator {
	turnIndex: number;
	actionIndex: number;
	player: number;
}

export class HistoryCursor {
	private readonly locators: PlayLocator[];
	private readonly initialState: ReplayState;
	/** State BEFORE the play at `cursor`. */
	private cachedPrev: ReplayState;
	private cursor: number;

	constructor(
		readonly history: GameHistory,
		readonly geometry: BoardGeometry
	) {
		this.locators = [];
		for (let ti = 0; ti < history.turns.length; ti++) {
			const turn = history.turns[ti];
			for (let ai = 0; ai < turn.actions.length; ai++) {
				if (turn.actions[ai].type === 'Play') {
					this.locators.push({ turnIndex: ti, actionIndex: ai, player: turn.player });
				}
			}
		}
		this.initialState = initialReplayState(history, geometry);
		this.cachedPrev = this.initialState;
		this.cursor = 0;
	}

	get length(): number {
		return this.locators.length;
	}
	get index(): number {
		return this.cursor;
	}
	get isAtStart(): boolean {
		return this.cursor === 0;
	}
	get isAtEnd(): boolean {
		return this.cursor >= this.locators.length;
	}
	get currentState(): ReplayState {
		return this.cachedPrev;
	}

	/** Advance one play. Returns the beat, or `null` at end. */
	stepForward(): PlayBeat | null {
		if (this.isAtEnd) return null;
		const i = this.cursor;
		const loc = this.locators[i];
		const prev = this.cachedPrev;
		const next = this.applyOnePlay(prev, i);
		const action = this.history.turns[loc.turnIndex].actions[loc.actionIndex];
		if (action.type !== 'Play') throw new Error('cursor locator pointed at non-Play');
		this.cachedPrev = next;
		this.cursor = i + 1;
		return { index: i, player: loc.player, record: action, prev, next };
	}

	/**
	 * Jump to the state BEFORE play `targetIndex` (`length` = past last
	 * play). O(n) from t=0 — good enough for interactive replay.
	 */
	jumpTo(targetIndex: number): ReplayState {
		const clamped = Math.max(0, Math.min(targetIndex, this.locators.length));
		let state = this.initialState;
		for (let i = 0; i < clamped; i++) {
			state = this.applyOnePlay(state, i);
		}
		this.cursor = clamped;
		this.cachedPrev = state;
		return state;
	}

	/** Back up one play. Returns the new state, or `null` at start. */
	stepBack(): ReplayState | null {
		if (this.isAtStart) return null;
		return this.jumpTo(this.cursor - 1);
	}

	/**
	 * Fold every history action from the previous play (or t=0) through
	 * the play at `locatorIndex` into `state`. Stamps turn metadata at
	 * turn boundaries so `current_player` / `turn_count` stay consistent
	 * through extra-turn chains (multiple Plays in one TurnRecord).
	 */
	private applyOnePlay(state: ReplayState, locatorIndex: number): ReplayState {
		const loc = this.locators[locatorIndex];
		const turn = this.history.turns[loc.turnIndex];

		let next = state;
		const crossesTurnBoundary =
			locatorIndex === 0 || this.locators[locatorIndex - 1].turnIndex !== loc.turnIndex;
		if (crossesTurnBoundary) {
			next = advanceToTurn(next, turn, loc.turnIndex);
		}

		const startActionIndex = crossesTurnBoundary
			? 0
			: this.locators[locatorIndex - 1].actionIndex + 1;
		for (let ai = startActionIndex; ai <= loc.actionIndex; ai++) {
			next = applyHistoryAction(next, turn.player, turn.actions[ai], this.geometry);
		}
		return next;
	}
}
