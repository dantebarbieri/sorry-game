import type { BoardGeometry, SpaceId } from '$lib/board/geometry';
import type { GameStateView } from '$lib/board/state';

/**
 * High-level reference to a space, resolved against a concrete
 * `BoardGeometry` at render time. This keeps demo specs readable:
 * `{ kind: 'track', index: 4 }` instead of a numeric SpaceId which
 * depends on how the engine enumerates spaces.
 */
export type SpaceRef =
	| { kind: 'track'; index: number }
	| { kind: 'start'; player: number }
	| { kind: 'safety'; player: number; index: number }
	| { kind: 'home'; player: number }
	| { kind: 'start_exit'; player: number };

export function resolveSpace(ref: SpaceRef, geom: BoardGeometry): SpaceId {
	if (ref.kind === 'start_exit') {
		const layout = geom.players[ref.player];
		if (!layout) throw new Error(`no player ${ref.player} in geometry`);
		return layout.start_exit;
	}
	for (const s of geom.spaces) {
		const k = s.kind;
		if (ref.kind === 'track' && 'Track' in k && k.Track === ref.index) return s.id;
		if (ref.kind === 'start' && 'StartArea' in k && k.StartArea === ref.player) return s.id;
		if (
			ref.kind === 'safety' &&
			'Safety' in k &&
			k.Safety[0] === ref.player &&
			k.Safety[1] === ref.index
		)
			return s.id;
		if (ref.kind === 'home' && 'Home' in k && k.Home === ref.player) return s.id;
	}
	throw new Error(`SpaceRef did not match any space: ${JSON.stringify(ref)}`);
}

export interface InitialPawnPlacement {
	player: number;
	pawn: number;
	/** If omitted, pawn stays at its player's StartArea. */
	at?: SpaceRef;
}

/**
 * Build a `GameStateView` with the given pawn placements. Any pawn not
 * explicitly placed defaults to its player's start area. Useful for
 * seeding demo scenes without replaying a real game history.
 */
export function makeInitialState(
	numPlayers: number,
	placements: InitialPawnPlacement[],
	geom: BoardGeometry,
	currentPlayer = 0
): GameStateView {
	const PAWNS_PER_PLAYER = 4;
	const pawn_positions: SpaceId[][] = [];
	for (let p = 0; p < numPlayers; p++) {
		const layout = geom.players[p];
		if (!layout) throw new Error(`geometry missing PlayerLayout for player ${p}`);
		const row: SpaceId[] = [];
		for (let pw = 0; pw < PAWNS_PER_PLAYER; pw++) row.push(layout.start_area);
		pawn_positions.push(row);
	}
	for (const placement of placements) {
		if (placement.at) {
			pawn_positions[placement.player][placement.pawn] = resolveSpace(placement.at, geom);
		}
	}
	return {
		num_players: numPlayers,
		pawn_positions,
		current_player: currentPlayer,
		turn_count: 0,
		drawn_card: null,
		deck_remaining: 45,
		discard: [],
		winners: [],
		truncated: false,
		current_turn: null,
		action_needed: { type: 'GameOver', winners: [], truncated: false }
	};
}
