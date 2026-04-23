import type { BoardGeometry } from '$lib/board/geometry';
import type { GameStateView } from '$lib/board/state';
import type { StepCommand } from '$lib/board/BoardCanvas.svelte';
import { findLastPlay, type GameHistory, type PlayerAction } from '$lib/board/actions';
import { loadWasm, parseJsonOrThrow } from '$lib/wasm';
import type { PlayController, PlaySetup } from './types';
import { DEFAULT_SETUP } from './types';

interface CreateResponse {
	game_id: number;
	state: GameStateView;
}
interface StateResponse {
	state: GameStateView;
}

export class LocalController implements PlayController {
	#geometry = $state<BoardGeometry | null>(null);
	#gameState = $state<GameStateView | null>(null);
	#lastStep = $state<StepCommand | undefined>(undefined);
	#error = $state<string | null>(null);
	#stepping = $state(false);
	#setup = $state<PlaySetup>(DEFAULT_SETUP);
	#gameId: number | null = null;

	get geometry() {
		return this.#geometry;
	}
	get gameState() {
		return this.#gameState;
	}
	get lastStep() {
		return this.#lastStep;
	}
	get error() {
		return this.#error;
	}
	get stepping() {
		return this.#stepping;
	}
	get setup() {
		return this.#setup;
	}
	get gameOver() {
		const s = this.#gameState;
		if (!s) return false;
		return s.winners.length > 0 || s.truncated;
	}

	async newGame(setup: PlaySetup = this.#setup) {
		this.#setup = setup;
		try {
			const wasm = await loadWasm();
			if (!this.#geometry) {
				this.#geometry = parseJsonOrThrow<BoardGeometry>(
					wasm.get_board_geometry(setup.rules)
				);
			}
			if (this.#gameId !== null) {
				wasm.destroy_interactive_game(this.#gameId);
				this.#gameId = null;
			}
			// Strategy name per seat: Human seats still need a placeholder in
			// the engine's strategy array (it's indexed by player) but will
			// never be invoked — human turns flow through `apply_action`.
			const strategy_names = setup.seats.map((s) =>
				s.type === 'Bot' ? s.strategy : 'Random'
			);
			const created = parseJsonOrThrow<CreateResponse>(
				wasm.create_interactive_game(
					JSON.stringify({
						strategy_names,
						seed: Math.floor(Math.random() * 1_000_000_000),
						rules: setup.rules
					})
				)
			);
			this.#gameId = created.game_id;
			this.#gameState = created.state;
			this.#lastStep = undefined;
			this.#error = null;
		} catch (e) {
			this.#error = e instanceof Error ? e.message : String(e);
		}
	}

	async #refreshAfterAction() {
		if (this.#gameId === null) return;
		const wasm = await loadWasm();
		const post = parseJsonOrThrow<StateResponse>(
			wasm.get_game_state(this.#gameId, -1)
		).state;
		const history = parseJsonOrThrow<GameHistory>(wasm.get_game_history(this.#gameId));
		const play = findLastPlay(history, post.current_turn);
		this.#gameState = post;
		if (play) {
			this.#lastStep = {
				record: play.record,
				player: play.player,
				nonce: (this.#lastStep?.nonce ?? 0) + 1
			};
		}
	}

	async commitAction(action: PlayerAction) {
		if (this.#gameId === null || this.#stepping) return;
		this.#stepping = true;
		try {
			const wasm = await loadWasm();
			parseJsonOrThrow<StateResponse>(
				wasm.apply_action(this.#gameId, JSON.stringify(action))
			);
			await this.#refreshAfterAction();
		} catch (e) {
			this.#error = e instanceof Error ? e.message : String(e);
		} finally {
			this.#stepping = false;
		}
	}

	async stepBot() {
		if (this.#gameId === null || this.#stepping || this.gameOver) return;
		const an = this.#gameState?.action_needed;
		if (an?.type !== 'ChooseMove' && an?.type !== 'ChooseCard') return;
		// Whichever seat is up, call its configured strategy. Human seats
		// never reach here because the UI gates `stepBot` on viewerCanMove.
		const seatIdx = an.player;
		const seat = this.#setup.seats[seatIdx];
		const strategy = seat?.type === 'Bot' ? seat.strategy : 'Random';
		this.#stepping = true;
		try {
			const wasm = await loadWasm();
			parseJsonOrThrow<StateResponse>(wasm.apply_bot_action(this.#gameId, strategy));
			await this.#refreshAfterAction();
		} catch (e) {
			this.#error = e instanceof Error ? e.message : String(e);
		} finally {
			this.#stepping = false;
		}
	}

	async passFor(seat: number) {
		const an = this.#gameState?.action_needed;
		if (an?.type !== 'ChooseMove') return;
		if (an.player !== seat) return;
		const pass = an.legal_moves.find((m) => m.type === 'Pass');
		if (!pass) return;
		await this.commitAction({ type: 'PlayMove', mv: pass });
	}

	destroy() {
		if (this.#gameId !== null) {
			void loadWasm().then((wasm) => {
				if (this.#gameId !== null) {
					wasm.destroy_interactive_game(this.#gameId);
					this.#gameId = null;
				}
			});
		}
	}
}
