import type { BoardGeometry, PlayerId, SpaceId } from './geometry';

const MAX_HOP_STEPS = 30; // safety bound; standard moves never exceed ~14

/**
 * Walk the per-player adjacency graph from `from` to `to` and return the
 * ordered sequence of spaces visited (inclusive of both endpoints). Returns
 * `null` if `to` isn't reachable within `MAX_HOP_STEPS` in the given
 * direction — the caller can fall back to a teleport.
 */
export function hopPath(
	geometry: BoardGeometry,
	player: PlayerId,
	from: SpaceId,
	to: SpaceId,
	direction: 'forward' | 'backward'
): SpaceId[] | null {
	const path: SpaceId[] = [from];
	if (from === to) return path;
	let current = from;
	for (let i = 0; i < MAX_HOP_STEPS; i++) {
		const layout = geometry.spaces.find((s) => s.id === current);
		if (!layout) return null;
		const neighbors = direction === 'forward' ? layout.forward : layout.backward;
		const next = neighbors[player];
		if (next == null) return null;
		path.push(next);
		if (next === to) return path;
		current = next;
	}
	return null;
}
