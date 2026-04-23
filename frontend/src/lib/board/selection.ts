import type { Move, PlayerAction, SplitLeg } from './actions';
import type { BoardGeometry, PlayerId, SpaceId } from './geometry';
import { hopPath } from './paths';

/**
 * Pure helpers for mapping raw clicks + a legal-move set → concrete
 * `Move`s. No DOM or three.js imports — these can be unit-tested in
 * isolation and reused by keyboard and mouse input alike.
 */

/** Pawns that appear as a movable piece in at least one legal move. Excludes
 *  Pass, and deduplicates (player, pawn) pairs. */
export function pickablePawns(
	currentPlayer: PlayerId,
	legalMoves: Move[]
): Array<{ player: PlayerId; pawn: number }> {
	const seen = new Set<string>();
	const out: Array<{ player: PlayerId; pawn: number }> = [];
	const push = (player: PlayerId, pawn: number) => {
		const key = `${player}:${pawn}`;
		if (seen.has(key)) return;
		seen.add(key);
		out.push({ player, pawn });
	};
	for (const mv of legalMoves) {
		switch (mv.type) {
			case 'Advance':
			case 'Retreat':
			case 'StartPawn':
				push(currentPlayer, mv.pawn);
				break;
			case 'Sorry':
				push(currentPlayer, mv.my_pawn);
				break;
			case 'SwapEleven':
				push(currentPlayer, mv.my_pawn);
				break;
			case 'SplitSeven':
				push(currentPlayer, mv.first.pawn);
				push(currentPlayer, mv.second.pawn);
				break;
			case 'Pass':
				break;
		}
	}
	return out;
}

/**
 * Destinations the renderer should highlight when `pawn` is selected.
 * Includes Advance / Retreat / StartPawn / Sorry `to` fields directly, and
 * resolves SwapEleven targets via `pawnPositions` (the swap move itself
 * doesn't carry a `to` — the target space is wherever the opponent pawn
 * currently sits). SplitSeven is multi-pick and isn't handled here.
 */
export function legalDestinationsForPawn(
	legalMoves: Move[],
	pawn: number,
	pawnPositions?: SpaceId[][]
): SpaceId[] {
	const out: SpaceId[] = [];
	const seen = new Set<SpaceId>();
	const push = (id: SpaceId) => {
		if (seen.has(id)) return;
		seen.add(id);
		out.push(id);
	};
	for (const mv of legalMoves) {
		switch (mv.type) {
			case 'Advance':
			case 'Retreat':
			case 'StartPawn':
				if (mv.pawn === pawn) push(mv.to);
				break;
			case 'Sorry':
				if (mv.my_pawn === pawn) push(mv.to);
				break;
			case 'SwapEleven':
				if (mv.my_pawn === pawn && pawnPositions) {
					const space = pawnPositions[mv.their_player]?.[mv.their_pawn];
					if (space !== undefined) push(space);
				}
				break;
			case 'SplitSeven':
				if (mv.first.pawn === pawn) push(mv.first.to);
				break;
			default:
				break;
		}
	}
	return out;
}

/** Find a SplitSeven move whose *first* leg matches (pawn, space), and
 *  return that leg for the UI to lock. Multiple matches can exist (each
 *  corresponds to a different choice of second leg) — any will do, since
 *  they all share the same first leg. */
export function matchSplitFirstLeg(
	legalMoves: Move[],
	pawn: number,
	space: SpaceId
): SplitLeg | null {
	for (const mv of legalMoves) {
		if (mv.type === 'SplitSeven' && mv.first.pawn === pawn && mv.first.to === space) {
			return mv.first;
		}
	}
	return null;
}

/** Given a locked-in first leg and a second pawn+space click, find the
 *  full SplitSeven move if legal. */
export function matchSplitSecondLeg(
	legalMoves: Move[],
	first: SplitLeg,
	pawn: number,
	space: SpaceId
): PlayerAction | null {
	for (const mv of legalMoves) {
		if (
			mv.type === 'SplitSeven' &&
			mv.first.pawn === first.pawn &&
			mv.first.to === first.to &&
			mv.second.pawn === pawn &&
			mv.second.to === space
		) {
			return { type: 'PlayMove', mv };
		}
	}
	return null;
}

/** Destinations to highlight for the *second* leg once `first` is locked
 *  and the user has selected a candidate second pawn. */
export function legalSecondLegDestinations(
	legalMoves: Move[],
	first: SplitLeg,
	secondPawn: number
): SpaceId[] {
	const out: SpaceId[] = [];
	const seen = new Set<SpaceId>();
	for (const mv of legalMoves) {
		if (
			mv.type === 'SplitSeven' &&
			mv.first.pawn === first.pawn &&
			mv.first.to === first.to &&
			mv.second.pawn === secondPawn
		) {
			if (!seen.has(mv.second.to)) {
				seen.add(mv.second.to);
				out.push(mv.second.to);
			}
		}
	}
	return out;
}

/**
 * Match a single-pick click sequence (pawn + destination) to a concrete
 * `PlayerAction`. Returns null when no such move exists in the legal set
 * (caller should treat as an illegal click).
 *
 * Multi-pick moves (`Sorry`, `SwapEleven`, `SplitSeven`) are not matched
 * here — they require additional picks and are owned by a separate
 * multi-pick flow that's wired up in a later pass.
 */
export function matchSinglePickMove(
	legalMoves: Move[],
	pawn: number,
	space: SpaceId
): PlayerAction | null {
	for (const mv of legalMoves) {
		switch (mv.type) {
			case 'Advance':
			case 'Retreat':
			case 'StartPawn':
				if (mv.pawn === pawn && mv.to === space) return { type: 'PlayMove', mv };
				break;
			case 'Sorry':
				// Sorry needs an opponent-pawn pick too; skip here.
				break;
			default:
				break;
		}
	}
	return null;
}

/**
 * Second-pick matcher for moves whose target is a specific opponent pawn:
 * `Sorry!` and `SwapEleven`. Call after the user has already selected one
 * of their own pawns and clicks a pawn belonging to another player.
 */
export function matchOpponentPickMove(
	legalMoves: Move[],
	myPawn: number,
	theirPlayer: PlayerId,
	theirPawn: number
): PlayerAction | null {
	for (const mv of legalMoves) {
		if (
			(mv.type === 'Sorry' || mv.type === 'SwapEleven') &&
			mv.my_pawn === myPawn &&
			mv.their_player === theirPlayer &&
			mv.their_pawn === theirPawn
		) {
			return { type: 'PlayMove', mv };
		}
	}
	return null;
}

/** True when `Pass` is a legal option — may or may not be the only one.
 *  The HUD surfaces a Pass button whenever this is true so the user can
 *  elect to forfeit the turn (notably on an 11 when Advance-11 is
 *  blocked, per the "may swap" rule). */
export function passIsLegal(legalMoves: Move[]): boolean {
	return legalMoves.some((m) => m.type === 'Pass');
}

/**
 * Forward-step distance from `from` to `to` along the player's path, or
 * `Number.MAX_SAFE_INTEGER` if unreachable. Used to order destination
 * highlights "around the board" so Tab/arrow keys traverse them in a
 * natural order rather than whatever order `legal_moves` emitted.
 */
export function forwardDistance(
	geometry: BoardGeometry,
	player: PlayerId,
	from: SpaceId,
	to: SpaceId
): number {
	if (from === to) return 0;
	const path = hopPath(geometry, player, from, to, 'forward');
	return path ? path.length - 1 : Number.MAX_SAFE_INTEGER;
}

/**
 * Sort destinations by forward-step distance from the moving pawn (or
 * from the owner's `start_exit` if the pawn is in Start — Sorry! — so
 * "closest" is measured from where the pawn would emerge).
 */
export function sortDestinationsByDistance(
	geometry: BoardGeometry,
	player: PlayerId,
	myPawnSpace: SpaceId,
	destinations: SpaceId[]
): SpaceId[] {
	const layout = geometry.spaces.find((s) => s.id === myPawnSpace);
	let reference = myPawnSpace;
	if (layout && 'StartArea' in layout.kind) {
		const playerLayout = geometry.players.find((p) => p.player === player);
		if (playerLayout) reference = playerLayout.start_exit;
	}
	return destinations
		.map((space) => ({ space, dist: forwardDistance(geometry, player, reference, space) }))
		.sort((a, b) => a.dist - b.dist)
		.map((entry) => entry.space);
}

/** True when the ONLY legal move is `Pass` (i.e. a forced forfeit). */
export function onlyPassIsLegal(legalMoves: Move[]): boolean {
	return legalMoves.length === 1 && legalMoves[0].type === 'Pass';
}
