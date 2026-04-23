import type { BoardGeometry, SpaceId } from '$lib/board/geometry';
import type { PlayRecord, Move, BumpEvent, SlideEvent } from '$lib/board/actions';
import type { CameraView } from '$lib/board/renderer';
import { makeInitialState, resolveSpace, type InitialPawnPlacement, type SpaceRef } from './synthetic-state';
import type { GameStateView } from '$lib/board/state';

/** A single step in a card demo. The `move` is a high-level spec resolved
 *  to a concrete `PlayRecord` against the geometry at render time. */
export interface DemoBeat {
	label: string;
	player: number;
	card: string;
	move: MoveSpec;
	/** Optional bumps; most demos omit. */
	bumps?: BumpSpec[];
	/** Optional slides; most demos omit. */
	slides?: SlideSpec[];
}

export type MoveSpec =
	| { type: 'Advance'; pawn: number; cardValue: number; to: SpaceRef }
	| { type: 'Retreat'; pawn: number; cardValue: number; to: SpaceRef }
	| { type: 'StartPawn'; pawn: number; to: SpaceRef }
	| { type: 'Sorry'; myPawn: number; theirPlayer: number; theirPawn: number; to: SpaceRef }
	| {
			type: 'SwapEleven';
			myPawn: number;
			theirPlayer: number;
			theirPawn: number;
		}
	| {
			type: 'SplitSeven';
			first: { pawn: number; steps: number; to: SpaceRef };
			second: { pawn: number; steps: number; to: SpaceRef };
		};

export interface BumpSpec {
	player: number;
	pawn: number;
	from: SpaceRef;
	to: SpaceRef;
}

export interface SlideSpec {
	player: number;
	pawn: number;
	from: SpaceRef;
	to: SpaceRef;
	path: SpaceRef[];
}

export interface DemoSpec {
	id: string;
	title: string;
	body: string;
	cameraView?: CameraView;
	/** Azimuth override for `edge`/`corner`; measured in radians. */
	cameraAzimuth?: number;
	/** How many players the scene models (always 4 for Sorry). */
	numPlayers: number;
	/** Initial pawn positions. Unspecified pawns default to their Start. */
	initialPlacements: InitialPawnPlacement[];
	/** The player who starts "up" in the demo. */
	startingPlayer: number;
	beats: DemoBeat[];
}

function resolveMove(spec: MoveSpec, geom: BoardGeometry): Move {
	switch (spec.type) {
		case 'Advance':
			return { type: 'Advance', pawn: spec.pawn, card_value: spec.cardValue, to: resolveSpace(spec.to, geom) };
		case 'Retreat':
			return { type: 'Retreat', pawn: spec.pawn, card_value: spec.cardValue, to: resolveSpace(spec.to, geom) };
		case 'StartPawn':
			return { type: 'StartPawn', pawn: spec.pawn, to: resolveSpace(spec.to, geom) };
		case 'Sorry':
			return {
				type: 'Sorry',
				my_pawn: spec.myPawn,
				their_player: spec.theirPlayer,
				their_pawn: spec.theirPawn,
				to: resolveSpace(spec.to, geom)
			};
		case 'SwapEleven':
			return {
				type: 'SwapEleven',
				my_pawn: spec.myPawn,
				their_player: spec.theirPlayer,
				their_pawn: spec.theirPawn
			};
		case 'SplitSeven':
			return {
				type: 'SplitSeven',
				first: {
					pawn: spec.first.pawn,
					steps: spec.first.steps,
					to: resolveSpace(spec.first.to, geom)
				},
				second: {
					pawn: spec.second.pawn,
					steps: spec.second.steps,
					to: resolveSpace(spec.second.to, geom)
				}
			};
	}
}

function resolveBump(spec: BumpSpec, geom: BoardGeometry): BumpEvent {
	return {
		player: spec.player,
		pawn: spec.pawn,
		from: resolveSpace(spec.from, geom),
		to: resolveSpace(spec.to, geom)
	};
}

function resolveSlide(spec: SlideSpec, geom: BoardGeometry): SlideEvent {
	return {
		player: spec.player,
		pawn: spec.pawn,
		from: resolveSpace(spec.from, geom),
		to: resolveSpace(spec.to, geom),
		path: spec.path.map((r) => resolveSpace(r, geom))
	};
}

export function resolveBeat(beat: DemoBeat, geom: BoardGeometry): PlayRecord {
	return {
		type: 'Play',
		card: beat.card,
		mv: resolveMove(beat.move, geom),
		bumps: (beat.bumps ?? []).map((b) => resolveBump(b, geom)),
		slides: (beat.slides ?? []).map((s) => resolveSlide(s, geom))
	};
}

/**
 * Apply a `PlayRecord` to a state (simplified — we don't need the full
 * replay reducer here because demos don't care about deck/discard/turns).
 * Returns the new `pawn_positions`.
 */
export function applyResolvedBeat(
	state: GameStateView,
	player: number,
	play: PlayRecord
): GameStateView {
	const pp = state.pawn_positions.map((row) => row.slice());
	const mv = play.mv;
	switch (mv.type) {
		case 'Advance':
		case 'Retreat':
		case 'StartPawn':
			pp[player][mv.pawn] = mv.to;
			break;
		case 'Sorry':
			pp[player][mv.my_pawn] = mv.to;
			break;
		case 'SwapEleven': {
			const a = pp[player][mv.my_pawn];
			const b = pp[mv.their_player][mv.their_pawn];
			pp[player][mv.my_pawn] = b;
			pp[mv.their_player][mv.their_pawn] = a;
			break;
		}
		case 'SplitSeven':
			pp[player][mv.first.pawn] = mv.first.to;
			pp[player][mv.second.pawn] = mv.second.to;
			break;
	}
	for (const bump of play.bumps) pp[bump.player][bump.pawn] = bump.to;
	for (const slide of play.slides) pp[slide.player][slide.pawn] = slide.to;
	return {
		...state,
		pawn_positions: pp,
		discard: [...state.discard, play.card]
	};
}

/**
 * Build all intermediate states for a spec: index 0 is the initial state,
 * index k is the state after the k-th beat has been applied. So
 * `states.length === beats.length + 1`.
 */
export function buildDemoStates(spec: DemoSpec, geom: BoardGeometry): {
	states: GameStateView[];
	resolvedBeats: PlayRecord[];
} {
	const initial = makeInitialState(
		spec.numPlayers,
		spec.initialPlacements,
		geom,
		spec.startingPlayer
	);
	const states: GameStateView[] = [initial];
	const resolved: PlayRecord[] = [];
	let cur = initial;
	for (const beat of spec.beats) {
		const play = resolveBeat(beat, geom);
		cur = applyResolvedBeat(cur, beat.player, play);
		states.push(cur);
		resolved.push(play);
	}
	return { states, resolvedBeats: resolved };
}

// ─── Demo library ─────────────────────────────────────────────────────
//
// Each demo is one or a few beats illustrating a single rule or card.
// Add new demos here; the rules page renders them by id.

export const DEMO_SETUP: DemoSpec = {
	id: 'setup',
	title: 'Setup',
	body: 'Every player starts with 4 pawns on their Start space. Colors go around clockwise: Red, Blue, Yellow, Green.',
	cameraView: 'top',
	numPlayers: 4,
	initialPlacements: [],
	startingPlayer: 0,
	beats: []
};

export const DEMO_ONE: DemoSpec = {
	id: 'card-one',
	title: 'The 1 card',
	body: 'A 1 lets you start a pawn out of Start onto your start-exit square, or move a pawn on the track one space forward.',
	cameraView: 'edge',
	cameraAzimuth: Math.PI,
	numPlayers: 4,
	initialPlacements: [],
	startingPlayer: 0,
	beats: [
		{
			label: 'Red plays a 1 to start a pawn',
			player: 0,
			card: 'One',
			move: { type: 'StartPawn', pawn: 0, to: { kind: 'start_exit', player: 0 } }
		}
	]
};

export const DEMO_FOUR: DemoSpec = {
	id: 'card-four',
	title: 'The 4 card (move backwards)',
	body: "A 4 moves a pawn four spaces backwards — it's the only card that forces you to retreat. Sneaky strategy: if your pawn is near your Start, a 4 wraps around and lands you deep in the track, much closer to Home.",
	cameraView: 'edge',
	cameraAzimuth: Math.PI,
	numPlayers: 4,
	initialPlacements: [{ player: 0, pawn: 0, at: { kind: 'start_exit', player: 0 } }],
	startingPlayer: 0,
	beats: [
		{
			label: 'Red plays a 4 — pawn retreats 4 spaces',
			player: 0,
			card: 'Four',
			move: {
				type: 'Retreat',
				pawn: 0,
				cardValue: 4,
				to: { kind: 'track', index: 0 }
			}
		}
	]
};

export const DEMO_ELEVEN_SWAP: DemoSpec = {
	id: 'card-eleven-swap',
	title: 'The 11 (swap)',
	body: 'An 11 can advance a pawn 11 spaces forward, OR swap any one of your pawns with any opponent pawn on the main track. Safety and Start pawns cannot be swapped.',
	cameraView: 'corner',
	numPlayers: 4,
	initialPlacements: [
		{ player: 0, pawn: 0, at: { kind: 'track', index: 10 } },
		{ player: 2, pawn: 0, at: { kind: 'track', index: 40 } }
	],
	startingPlayer: 0,
	beats: [
		{
			label: 'Red plays an 11 and swaps pawns with Yellow',
			player: 0,
			card: 'Eleven',
			move: { type: 'SwapEleven', myPawn: 0, theirPlayer: 2, theirPawn: 0 }
		}
	]
};

export const DEMO_SORRY: DemoSpec = {
	id: 'card-sorry',
	title: 'The Sorry! card',
	body: 'A Sorry! card teleports a pawn from your Start area onto the exact space of any opponent pawn on the main track, sending that pawn back to their Start. A satisfying equalizer.',
	cameraView: 'corner',
	numPlayers: 4,
	initialPlacements: [{ player: 1, pawn: 0, at: { kind: 'track', index: 28 } }],
	startingPlayer: 0,
	beats: [
		{
			label: 'Red says Sorry! to Blue',
			player: 0,
			card: 'Sorry',
			move: {
				type: 'Sorry',
				myPawn: 0,
				theirPlayer: 1,
				theirPawn: 0,
				to: { kind: 'track', index: 28 }
			},
			bumps: [
				{
					player: 1,
					pawn: 0,
					from: { kind: 'track', index: 28 },
					to: { kind: 'start', player: 1 }
				}
			]
		}
	]
};

export const DEMO_SEVEN_SPLIT: DemoSpec = {
	id: 'card-seven',
	title: 'The 7 card (split)',
	body: 'A 7 moves one pawn 7 spaces forward, OR splits its 7 points between two of your pawns — any way the numbers break (1+6, 2+5, 3+4). You cannot use a 7 to start a pawn out of Start. In this demo, Red splits 7 into 3 + 4 — and the +4 pawn lands on the head of a Blue slide, riding it four spaces further.',
	cameraView: 'corner',
	numPlayers: 4,
	initialPlacements: [
		{ player: 0, pawn: 0, at: { kind: 'track', index: 4 } },
		{ player: 0, pawn: 1, at: { kind: 'track', index: 20 } }
	],
	startingPlayer: 0,
	beats: [
		{
			label: 'Red splits 7 into 3 + 4; the +4 lands on Blue’s slide',
			player: 0,
			card: 'Seven',
			move: {
				type: 'SplitSeven',
				first: { pawn: 0, steps: 3, to: { kind: 'track', index: 7 } },
				second: { pawn: 1, steps: 4, to: { kind: 'track', index: 24 } }
			},
			slides: [
				{
					player: 0,
					pawn: 1,
					from: { kind: 'track', index: 24 },
					to: { kind: 'track', index: 28 },
					path: [
						{ kind: 'track', index: 25 },
						{ kind: 'track', index: 26 },
						{ kind: 'track', index: 27 },
						{ kind: 'track', index: 28 }
					]
				}
			]
		}
	]
};

export const ALL_DEMOS: DemoSpec[] = [
	DEMO_SETUP,
	DEMO_ONE,
	DEMO_FOUR,
	DEMO_SEVEN_SPLIT,
	DEMO_ELEVEN_SWAP,
	DEMO_SORRY
];
