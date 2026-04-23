// Batch simulation worker. Runs games one at a time, yielding between each
// so the worker can service `pause` / `stop` messages without tearing
// through the whole batch. Posts `progress` no more often than every
// ~50 ms to keep the main thread responsive.
//
// Message protocol: `src/lib/simulate/types.ts`.

/// <reference lib="webworker" />

import type { GameHistory } from './board/actions';
import type {
	GameSummary,
	ProgressStats,
	SimConfig,
	WorkerRequest,
	WorkerResponse
} from './simulate/types';
import { loadWorkerWasm, parseJsonOrThrow } from './wasm-worker';

interface SingleGameResult {
	stats: { winners: number[]; num_turns: number; truncated: boolean };
	history: GameHistory;
}

const ctx: DedicatedWorkerGlobalScope = self as unknown as DedicatedWorkerGlobalScope;

let paused = false;
let stopped = false;
let pauseWaiter: Promise<void> = Promise.resolve();
let releasePause: (() => void) | null = null;

function post(msg: WorkerResponse) {
	ctx.postMessage(msg);
}

function setPaused(p: boolean) {
	if (p === paused) return;
	paused = p;
	if (p) {
		pauseWaiter = new Promise<void>((resolve) => {
			releasePause = resolve;
		});
	} else {
		releasePause?.();
		releasePause = null;
	}
}

ctx.addEventListener('message', (e: MessageEvent<WorkerRequest>) => {
	const msg = e.data;
	switch (msg.type) {
		case 'start':
			stopped = false;
			setPaused(false);
			void runBatch(msg.config);
			break;
		case 'pause':
			setPaused(true);
			break;
		case 'resume':
			setPaused(false);
			break;
		case 'stop':
			stopped = true;
			setPaused(false);
			break;
	}
});

void loadWorkerWasm().then(
	() => post({ type: 'ready' }),
	(e) => post({ type: 'error', message: `wasm load failed: ${e instanceof Error ? e.message : e}` })
);

async function runBatch(config: SimConfig) {
	try {
		const wasm = await loadWorkerWasm();
		const numPlayers = config.strategies.length;
		// placements[p][rank] counts games where player p finished at rank.
		// rank 0 = 1st place, 1 = 2nd, etc. Standard rules only populate
		// rank 0; PlayOut fills up to rank numPlayers-1.
		const placements: number[][] = Array.from({ length: numPlayers }, () =>
			new Array<number>(numPlayers).fill(0)
		);
		let turnSum = 0;
		let minTurns = Infinity;
		let maxTurns = 0;
		let truncatedCount = 0;
		const histories: GameHistory[] = [];
		let newSinceTick: GameSummary[] = [];
		let lastTickAt = performance.now();

		for (let i = 0; i < config.num_games; i++) {
			if (stopped) break;
			if (paused) await pauseWaiter;
			if (stopped) break;

			const seed = (config.base_seed + i) >>> 0;
			const raw = wasm.simulate_one_with_history(
				JSON.stringify({
					seed,
					strategies: config.strategies,
					rules: config.rules,
					max_turns: config.max_turns
				})
			);
			const res = parseJsonOrThrow<SingleGameResult>(raw);

			for (let rank = 0; rank < res.stats.winners.length; rank++) {
				const p = res.stats.winners[rank];
				if (p < numPlayers && rank < numPlayers) placements[p][rank]++;
			}
			turnSum += res.stats.num_turns;
			if (res.stats.num_turns < minTurns) minTurns = res.stats.num_turns;
			if (res.stats.num_turns > maxTurns) maxTurns = res.stats.num_turns;
			if (res.stats.truncated) truncatedCount++;
			const summary: GameSummary = {
				index: i,
				seed,
				winners: res.stats.winners.slice(),
				num_turns: res.stats.num_turns,
				truncated: res.stats.truncated
			};
			newSinceTick.push(summary);
			if (config.keep_histories) histories.push(res.history);

			const now = performance.now();
			if (now - lastTickAt >= 50 || i === config.num_games - 1) {
				post({
					type: 'progress',
					stats: buildProgressStats(
						i + 1,
						config.num_games,
						placements,
						turnSum,
						minTurns,
						maxTurns,
						truncatedCount
					),
					new_games: newSinceTick
				});
				newSinceTick = [];
				lastTickAt = now;
			}

			// Yield to the worker event loop so pause/stop messages arrive.
			await new Promise<void>((resolve) => setTimeout(resolve, 0));
		}

		const finalStats = buildProgressStats(
			config.num_games,
			config.num_games,
			placements,
			turnSum,
			minTurns,
			maxTurns,
			truncatedCount
		);
		if (stopped) {
			post({ type: 'stopped', stats: finalStats });
		} else {
			post({
				type: 'complete',
				stats: finalStats,
				histories: config.keep_histories ? histories : undefined
			});
		}
	} catch (e) {
		post({ type: 'error', message: e instanceof Error ? e.message : String(e) });
	}
}

function buildProgressStats(
	played: number,
	total: number,
	placements: number[][],
	turnSum: number,
	minTurns: number,
	maxTurns: number,
	truncatedCount: number
): ProgressStats {
	const safePlayed = played || 1;
	return {
		games_played: played,
		total_games: total,
		placements: placements.map((row) => row.slice()),
		placement_rate: placements.map((row) => row.map((c) => c / safePlayed)),
		avg_turns: turnSum / safePlayed,
		min_turns: minTurns === Infinity ? 0 : minTurns,
		max_turns: maxTurns,
		truncated_count: truncatedCount
	};
}
