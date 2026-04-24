// Typed wrapper for the room WebSocket. Mirrors the
// `ClientMessage` / `ServerMessage` unions in
// `sorry-server/src/messages.rs` exactly — if the server changes shapes,
// TypeScript here is where it should fail first.

import type { GameStateView } from '$lib/board/state';
import type { PlayerAction } from '$lib/board/actions';

export interface LobbyPlayer {
	slot: number;
	name: string;
	player_type: PlayerSlotType;
	connected: boolean;
	disconnect_secs?: number;
}

export type PlayerSlotType =
	| { kind: 'Human' }
	| { kind: 'Bot'; strategy: string }
	| { kind: 'Empty' };

export interface LobbySpectator {
	idx: number;
	name: string;
	connected: boolean;
	disconnect_secs?: number;
}

export interface RoomLobbyState {
	room_code: string;
	phase: string;
	players: LobbyPlayer[];
	spectators: LobbySpectator[];
	num_players: number;
	rules: string;
	creator: number;
	available_strategies: string[];
	available_rules: string[];
	idle_timeout_secs: number | null;
	turn_timer_secs: number | null;
	last_winners: number[];
}

/** Server-authoritative per-player view. Shape matches `PlayerView` in
 *  `sorry-core`; compatible with `GameStateView` for the renderer after
 *  `playerViewToGameState`. */
export interface PlayerView extends GameStateView {
	viewer: number;
	starting_player: number;
	/** `hands[p]` is non-null only for `p == viewer`. */
	hands: (string[] | null)[];
	rules_name: string;
	strategy_names: string[];
}

export type ClientMessage =
	| { type: 'ConfigureSlot'; slot: number; player_type: string }
	| { type: 'SetNumPlayers'; num_players: number }
	| { type: 'SetRules'; rules: string }
	| { type: 'KickPlayer'; slot: number }
	| { type: 'PromoteHost'; slot: number }
	| { type: 'SetTurnTimer'; secs: number | null }
	| { type: 'StartGame' }
	| { type: 'Action'; action: PlayerAction }
	| { type: 'PlayAgain' }
	| { type: 'ReturnToLobby' }
	| { type: 'TakeSlot'; slot: number }
	| { type: 'BecomeSpectator' }
	| { type: 'Ping' };

export type ServerMessage =
	| { type: 'RoomState'; state: RoomLobbyState }
	| { type: 'GameState'; state: PlayerView; turn_deadline_secs?: number }
	| {
			type: 'ActionApplied';
			player: number;
			action: PlayerAction;
			state: PlayerView;
			turn_deadline_secs?: number;
	  }
	| {
			type: 'BotAction';
			player: number;
			action: PlayerAction;
			state: PlayerView;
			turn_deadline_secs?: number;
	  }
	| { type: 'TimeoutAction'; player: number; action: PlayerAction; state: PlayerView }
	| { type: 'PlayerJoined'; player_index: number; name: string }
	| { type: 'PlayerLeft'; player_index: number }
	| { type: 'PlayerReconnected'; player_index: number }
	| { type: 'Kicked'; reason: string }
	| { type: 'Error'; code: string; message: string }
	| { type: 'Pong' };

export type WsStatus = 'connecting' | 'open' | 'closed' | 'error';

export interface WsClientOptions {
	url: string;
	onMessage: (msg: ServerMessage) => void;
	onStatusChange?: (status: WsStatus) => void;
	/** Ms between `Ping` frames. Set to 0 to disable. */
	heartbeatMs?: number;
}

/** Thin WebSocket wrapper with exponential-backoff reconnect + heartbeat. */
export class RoomWsClient {
	#url: string;
	#sock: WebSocket | null = null;
	#onMessage: (msg: ServerMessage) => void;
	#onStatus: ((s: WsStatus) => void) | undefined;
	#heartbeatMs: number;
	#heartbeat: ReturnType<typeof setInterval> | null = null;
	#reconnectAttempt = 0;
	#disposed = false;

	constructor(opts: WsClientOptions) {
		this.#url = opts.url;
		this.#onMessage = opts.onMessage;
		this.#onStatus = opts.onStatusChange;
		this.#heartbeatMs = opts.heartbeatMs ?? 15_000;
	}

	connect() {
		if (this.#disposed) return;
		this.#onStatus?.('connecting');
		const sock = new WebSocket(this.#url);
		this.#sock = sock;
		sock.onopen = () => {
			this.#reconnectAttempt = 0;
			this.#onStatus?.('open');
			this.#startHeartbeat();
		};
		sock.onmessage = (ev) => {
			try {
				const msg = JSON.parse(ev.data as string) as ServerMessage;
				this.#onMessage(msg);
			} catch (e) {
				console.warn('[ws] malformed message', ev.data, e);
			}
		};
		sock.onerror = () => this.#onStatus?.('error');
		sock.onclose = () => {
			this.#stopHeartbeat();
			this.#sock = null;
			this.#onStatus?.('closed');
			if (!this.#disposed) {
				this.#reconnectAttempt++;
				const delay = Math.min(1000 * 2 ** Math.min(this.#reconnectAttempt, 5), 30_000);
				setTimeout(() => this.connect(), delay);
			}
		};
	}

	send(msg: ClientMessage) {
		if (this.#sock && this.#sock.readyState === WebSocket.OPEN) {
			this.#sock.send(JSON.stringify(msg));
		} else {
			console.warn('[ws] send dropped, socket not open', msg);
		}
	}

	dispose() {
		this.#disposed = true;
		this.#stopHeartbeat();
		this.#sock?.close();
		this.#sock = null;
	}

	#startHeartbeat() {
		this.#stopHeartbeat();
		if (this.#heartbeatMs <= 0) return;
		this.#heartbeat = setInterval(() => {
			this.send({ type: 'Ping' });
		}, this.#heartbeatMs);
	}
	#stopHeartbeat() {
		if (this.#heartbeat) {
			clearInterval(this.#heartbeat);
			this.#heartbeat = null;
		}
	}
}
