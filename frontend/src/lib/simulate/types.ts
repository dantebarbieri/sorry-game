import type { GameHistory } from '$lib/board/actions';

export interface SimConfig {
	num_games: number;
	rules: 'Standard' | 'PlayOut';
	/** Per-player strategy name. Length defines player count. */
	strategies: string[];
	/** Base seed. Game k uses `base_seed + k` so runs are reproducible. */
	base_seed: number;
	/** If set, a game is marked `truncated` after this many turns. */
	max_turns?: number;
	/** Keep per-game histories for later replay. Adds memory cost. */
	keep_histories: boolean;
}

export interface GameSummary {
	index: number;
	seed: number;
	winners: number[];
	num_turns: number;
	truncated: boolean;
}

export interface ProgressStats {
	games_played: number;
	total_games: number;
	/**
	 * Per-player placement counts. `placements[p][rank]` = number of
	 * games in which player p finished at `rank` (0 = 1st, 1 = 2nd, …).
	 * Standard Sorry! only fills `rank == 0`; PlayOut fills all four.
	 */
	placements: number[][];
	/** `placement_rate[p][rank] = placements[p][rank] / games_played`. */
	placement_rate: number[][];
	avg_turns: number;
	min_turns: number;
	max_turns: number;
	truncated_count: number;
}

/** Messages posted from the main thread to the worker. */
export type WorkerRequest =
	| { type: 'start'; config: SimConfig }
	| { type: 'pause' }
	| { type: 'resume' }
	| { type: 'stop' };

/** Messages posted from the worker back to the main thread. */
export type WorkerResponse =
	| { type: 'ready' }
	| {
			type: 'progress';
			stats: ProgressStats;
			/** Newly-completed game summaries since the previous progress tick. */
			new_games: GameSummary[];
	  }
	| {
			type: 'complete';
			stats: ProgressStats;
			/** Present only when `config.keep_histories` was true. */
			histories?: GameHistory[];
	  }
	| { type: 'stopped'; stats: ProgressStats }
	| { type: 'error'; message: string };
