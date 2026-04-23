import { describe, it, expect } from 'vitest';
import type { BoardGeometry } from '../../board/geometry';
import {
	ALL_DEMOS,
	applyResolvedBeat,
	buildDemoStates,
	resolveBeat
} from '../demo-specs';
import { makeInitialState, resolveSpace } from '../synthetic-state';

/**
 * Hand-rolled 4-player geometry stub that satisfies SpaceRef lookups:
 * - 60-space main track
 * - 4 start areas, 4 start_exits, 4 home spaces, 5 safety spaces per player
 *
 * IDs are assigned sequentially; exact values don't matter — the demo-spec
 * tests only check that refs resolve consistently and that `applyResolvedBeat`
 * moves the right pawn to the right SpaceId.
 */
function buildStubGeometry(): BoardGeometry {
	const spaces: BoardGeometry['spaces'] = [];
	let nextId = 0;
	const track: number[] = [];
	for (let i = 0; i < 60; i++) {
		const id = nextId++;
		track.push(id);
		spaces.push({
			id,
			kind: { Track: i },
			center: [0, 0],
			tangent_deg: 0,
			forward: [null, null, null, null],
			backward: [null, null, null, null]
		});
	}
	const starts: number[] = [];
	const homes: number[] = [];
	for (let p = 0; p < 4; p++) {
		const sid = nextId++;
		starts.push(sid);
		spaces.push({
			id: sid,
			kind: { StartArea: p },
			center: [0, 0],
			tangent_deg: 0,
			forward: [null, null, null, null],
			backward: [null, null, null, null]
		});
		for (let si = 0; si < 5; si++) {
			const sfid = nextId++;
			spaces.push({
				id: sfid,
				kind: { Safety: [p, si] },
				center: [0, 0],
				tangent_deg: 0,
				forward: [null, null, null, null],
				backward: [null, null, null, null]
			});
		}
		const hid = nextId++;
		homes.push(hid);
		spaces.push({
			id: hid,
			kind: { Home: p },
			center: [0, 0],
			tangent_deg: 0,
			forward: [null, null, null, null],
			backward: [null, null, null, null]
		});
	}
	return {
		bounds: [-1, -1, 1, 1],
		spaces,
		slides: [],
		players: [0, 1, 2, 3].map((p) => ({
			player: p,
			start_area: starts[p],
			start_exit: track[p * 15],
			home: homes[p]
		}))
	};
}

describe('resolveSpace', () => {
	const geom = buildStubGeometry();

	it('resolves track spaces', () => {
		expect(resolveSpace({ kind: 'track', index: 10 }, geom)).toBe(10);
	});

	it('resolves start areas', () => {
		expect(resolveSpace({ kind: 'start', player: 0 }, geom)).toBe(
			geom.players[0].start_area
		);
	});

	it('resolves start_exit', () => {
		expect(resolveSpace({ kind: 'start_exit', player: 2 }, geom)).toBe(
			geom.players[2].start_exit
		);
	});

	it('throws on an unknown ref', () => {
		expect(() =>
			resolveSpace({ kind: 'track', index: 999 }, geom)
		).toThrow();
	});
});

describe('makeInitialState', () => {
	const geom = buildStubGeometry();

	it('places every pawn on its player start by default', () => {
		const s = makeInitialState(4, [], geom);
		for (let p = 0; p < 4; p++) {
			for (let pw = 0; pw < 4; pw++) {
				expect(s.pawn_positions[p][pw]).toBe(geom.players[p].start_area);
			}
		}
	});

	it('applies explicit placements', () => {
		const s = makeInitialState(
			4,
			[{ player: 0, pawn: 1, at: { kind: 'track', index: 5 } }],
			geom
		);
		expect(s.pawn_positions[0][1]).toBe(5);
		expect(s.pawn_positions[0][0]).toBe(geom.players[0].start_area);
	});
});

describe('ALL_DEMOS', () => {
	const geom = buildStubGeometry();

	for (const spec of ALL_DEMOS) {
		it(`${spec.id} builds without error`, () => {
			const built = buildDemoStates(spec, geom);
			expect(built.states.length).toBe(spec.beats.length + 1);
			expect(built.resolvedBeats.length).toBe(spec.beats.length);
			for (const rb of built.resolvedBeats) {
				expect(rb.type).toBe('Play');
			}
		});
	}
});

describe('applyResolvedBeat', () => {
	const geom = buildStubGeometry();

	it('applies Advance to the right pawn', () => {
		const state = makeInitialState(
			4,
			[{ player: 0, pawn: 0, at: { kind: 'track', index: 4 } }],
			geom
		);
		const play = resolveBeat(
			{
				label: 'Advance 3',
				player: 0,
				card: 'Three',
				move: { type: 'Advance', pawn: 0, cardValue: 3, to: { kind: 'track', index: 7 } }
			},
			geom
		);
		const next = applyResolvedBeat(state, 0, play);
		expect(next.pawn_positions[0][0]).toBe(7);
		expect(next.discard).toContain('Three');
	});

	it('applies Sorry with a bump', () => {
		const state = makeInitialState(
			4,
			[{ player: 1, pawn: 0, at: { kind: 'track', index: 28 } }],
			geom
		);
		const play = resolveBeat(
			{
				label: 'Red bumps Blue',
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
			},
			geom
		);
		const next = applyResolvedBeat(state, 0, play);
		expect(next.pawn_positions[0][0]).toBe(28);
		expect(next.pawn_positions[1][0]).toBe(geom.players[1].start_area);
	});
});
