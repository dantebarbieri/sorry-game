import type { SpaceId } from './geometry';
import type { ActionNeeded, TurnRecord } from './actions';

/**
 * Minimal state shape consumed by the renderer. A subset of the Rust-side
 * `InteractiveGameState` / `PlayerView` — only what the renderer needs to
 * place pawns. Later phases expand this for animation and HUD hooks.
 */
export interface GameStateView {
	num_players: number;
	/** `pawn_positions[playerIdx][pawnIdx]` = current `SpaceId`. */
	pawn_positions: SpaceId[][];
	current_player: number;
	turn_count: number;
	/** Card currently drawn by the current player (null between turns). */
	drawn_card: string | null;
	/** Number of cards remaining in the face-down deck. */
	deck_remaining: number;
	/** Discard pile, oldest → newest; last element is the card most recently played. */
	discard: string[];
	/** Non-empty when the game has ended. */
	winners: number[];
	truncated: boolean;
	/**
	 * The turn currently being assembled. Actions accumulate here during
	 * an extra-turn chain (e.g. after playing a 2) and only move into
	 * `history.turns` once the turn finally ends. Consult this when
	 * hunting for the most recent `Play` — the finalized history alone
	 * will miss a just-played 2-card.
	 */
	current_turn: TurnRecord | null;
	action_needed: ActionNeeded;
}
