import type { BoardGeometry } from '$lib/board/geometry';
import type { GameStateView } from '$lib/board/state';
import type { StepCommand } from '$lib/board/BoardCanvas.svelte';
import type { PlayerAction } from '$lib/board/actions';
import { loadWasm, parseJsonOrThrow } from '$lib/wasm';
import type { PlayController, PlaySetup } from './types';
import { DEFAULT_SETUP } from './types';
import {
	RoomWsClient,
	type PlayerView,
	type RoomLobbyState,
	type ServerMessage
} from './ws-client';
import { wsUrl } from './rest-client';

function playerViewToGameState(pv: PlayerView): GameStateView {
	return {
		num_players: pv.num_players,
		pawn_positions: pv.pawn_positions,
		current_player: pv.current_player,
		turn_count: pv.turn_count,
		drawn_card: pv.drawn_card ?? null,
		deck_remaining: pv.deck_remaining,
		discard: pv.discard,
		winners: pv.winners,
		truncated: pv.truncated,
		current_turn: null,
		action_needed: pv.action_needed
	};
}

export class OnlineController implements PlayController {
	#ws: RoomWsClient | null = null;
	#geometry = $state<BoardGeometry | null>(null);
	#gameState = $state<GameStateView | null>(null);
	#error = $state<string | null>(null);
	#lobby = $state<RoomLobbyState | null>(null);
	#status = $state<'idle' | 'connecting' | 'connected' | 'closed'>('idle');
	#viewer = $state<number | null>(null);
	#setup = $state<PlaySetup>(DEFAULT_SETUP);

	get geometry() {
		return this.#geometry;
	}
	get gameState() {
		return this.#gameState;
	}
	get lastStep(): StepCommand | undefined {
		return undefined;
	}
	get error() {
		return this.#error;
	}
	get stepping() {
		return false;
	}
	get gameOver() {
		const s = this.#gameState;
		if (!s) return false;
		return s.winners.length > 0 || s.truncated;
	}
	get setup() {
		return this.#setup;
	}
	get lobby() {
		return this.#lobby;
	}
	get status() {
		return this.#status;
	}
	get viewer() {
		return this.#viewer;
	}

	async connect(baseUrl: string, code: string, token: string, viewer: number) {
		this.#viewer = viewer;
		this.#status = 'connecting';
		this.#error = null;
		// Load board geometry once — rules name comes from the server's
		// lobby state on first `RoomState` message. For Standard and
		// PlayOut the geometry is identical, so load Standard here and
		// trust that no variant with a different topology is active.
		try {
			const wasm = await loadWasm();
			this.#geometry = parseJsonOrThrow<BoardGeometry>(wasm.get_board_geometry('Standard'));
		} catch (e) {
			this.#error = e instanceof Error ? e.message : String(e);
			this.#status = 'closed';
			return;
		}

		this.#ws = new RoomWsClient({
			url: wsUrl(baseUrl, code, token),
			onMessage: (msg) => this.#onMessage(msg),
			onStatusChange: (s) => {
				if (s === 'open') this.#status = 'connected';
				else if (s === 'closed' || s === 'error') this.#status = 'closed';
			}
		});
		this.#ws.connect();
	}

	disconnect() {
		this.#ws?.dispose();
		this.#ws = null;
		this.#status = 'closed';
	}

	#onMessage(msg: ServerMessage) {
		switch (msg.type) {
			case 'RoomState':
				this.#lobby = msg.state;
				// Derive the play setup so it matches what the server is using.
				this.#setup = {
					rules: (msg.state.rules as PlaySetup['rules']) ?? 'Standard',
					seats: msg.state.players.map((p) => {
						if (p.player_type.kind === 'Human') return { type: 'Human' as const };
						if (p.player_type.kind === 'Bot')
							return { type: 'Bot' as const, strategy: p.player_type.strategy };
						return { type: 'Empty' as const };
					})
				};
				break;
			case 'GameState':
			case 'ActionApplied':
			case 'BotAction':
			case 'TimeoutAction':
				this.#gameState = playerViewToGameState(msg.state);
				break;
			case 'Error':
				this.#error = `${msg.code}: ${msg.message}`;
				break;
			case 'Kicked':
				this.#error = `Kicked: ${msg.reason}`;
				this.disconnect();
				break;
			// Lobby-only events are ignored here; the OnlineLobby component
			// re-reads `#lobby` on every `RoomState`.
		}
	}

	// ─── PlayController (interactive surface) ────────────────────────────

	async commitAction(action: PlayerAction) {
		this.#ws?.send({ type: 'Action', action });
	}

	async stepBot() {
		// No-op: the server plays bots server-side.
	}

	async newGame() {
		// Online "new game" = PlayAgain after a game ends (or ReturnToLobby).
		this.#ws?.send({ type: 'PlayAgain' });
	}

	async passFor(seat: number) {
		const an = this.#gameState?.action_needed;
		if (an?.type !== 'ChooseMove') return;
		if (an.player !== seat) return;
		const pass = an.legal_moves.find((m) => m.type === 'Pass');
		if (!pass) return;
		await this.commitAction({ type: 'PlayMove', mv: pass });
	}

	// ─── Lobby controls ──────────────────────────────────────────────────

	configureSlot(slot: number, playerType: string) {
		this.#ws?.send({ type: 'ConfigureSlot', slot, player_type: playerType });
	}
	setNumPlayers(n: number) {
		this.#ws?.send({ type: 'SetNumPlayers', num_players: n });
	}
	setRules(rules: string) {
		this.#ws?.send({ type: 'SetRules', rules });
	}
	kickPlayer(slot: number) {
		this.#ws?.send({ type: 'KickPlayer', slot });
	}
	promoteHost(slot: number) {
		this.#ws?.send({ type: 'PromoteHost', slot });
	}
	setTurnTimer(secs: number | null) {
		this.#ws?.send({ type: 'SetTurnTimer', secs });
	}
	startGame() {
		this.#ws?.send({ type: 'StartGame' });
	}
	returnToLobby() {
		this.#ws?.send({ type: 'ReturnToLobby' });
	}
}
