import type { BoardGeometry } from '$lib/board/geometry';
import type { GameStateView } from '$lib/board/state';
import type { PlayerAction } from '$lib/board/actions';
import type { StepCommand } from '$lib/board/BoardCanvas.svelte';

export type Ruleset = 'Standard' | 'PlayOut';

export type SeatKind = { type: 'Human' } | { type: 'Bot'; strategy: string };

export interface PlaySetup {
	rules: Ruleset;
	seats: SeatKind[];
}

/** Which seat the local device is currently rendering for.
 *  `null` = observer (no human control). */
export type ViewerSeat = number | null;

export const PLAYER_NAMES = ['Red', 'Blue', 'Yellow', 'Green'] as const;
export const PLACEMENT_ORDINAL = ['1st', '2nd', '3rd', '4th'] as const;

export const DEFAULT_SETUP: PlaySetup = {
	rules: 'Standard',
	seats: [
		{ type: 'Human' },
		{ type: 'Bot', strategy: 'Random' },
		{ type: 'Bot', strategy: 'Random' },
		{ type: 'Bot', strategy: 'Random' }
	]
};

/**
 * Shared controller contract. `LocalController` drives the WASM interactive
 * game in-memory; a future `OnlineController` will satisfy the same shape
 * but forward actions over WebSocket. The interactive UI components only
 * depend on this interface.
 */
export interface PlayController {
	readonly geometry: BoardGeometry | null;
	readonly gameState: GameStateView | null;
	readonly lastStep: StepCommand | undefined;
	readonly error: string | null;
	readonly stepping: boolean;
	readonly gameOver: boolean;
	readonly setup: PlaySetup;

	commitAction(action: PlayerAction): Promise<void>;
	stepBot(): Promise<void>;
	newGame(setup?: PlaySetup): Promise<void>;
	/** Convenience — commit a Pass for the given seat, if Pass is legal and
	 *  it's that seat's turn. No-op otherwise. */
	passFor(seat: number): Promise<void>;
}
