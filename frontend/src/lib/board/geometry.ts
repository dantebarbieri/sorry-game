// TS mirror of the Rust `BoardGeometry` published by `Rules::board_geometry()`
// and exposed via WASM `get_board_geometry(rules_name)`. These types must stay
// in sync with `sorry-core/src/geometry.rs`.
//
// Serialization notes (pinned by
// `sorry-core/src/rules.rs::board_geometry_json_shape_is_frontend_friendly`):
//   - `SpaceId(u32)` and `PlayerId(u8)` are newtype structs — they serialize
//     transparently as bare numbers.
//   - `Space` enum uses default externally-tagged form: `{"Track": 0}`,
//     `{"StartArea": 2}`, `{"Safety": [0, 3]}`, `{"Home": 1}`.

export type SpaceId = number;
export type PlayerId = number;

export type SpaceKind =
	| { Track: number }
	| { StartArea: PlayerId }
	| { Safety: [PlayerId, number] }
	| { Home: PlayerId };

export interface SpaceLayout {
	id: SpaceId;
	kind: SpaceKind;
	/** `[x, y]` in the board-local `[-1, 1]` square. */
	center: [number, number];
	/** Pawn facing direction, counter-clockwise from `+x` in degrees. */
	tangent_deg: number;
	/** Indexed by `PlayerId`. `null` means no neighbor for that player. */
	forward: (SpaceId | null)[];
	/** Indexed by `PlayerId`. `null` means no neighbor for that player. */
	backward: (SpaceId | null)[];
}

export interface SlideLayout {
	head: SpaceId;
	end: SpaceId;
	path: SpaceId[];
	owner: PlayerId;
}

export interface PlayerLayout {
	player: PlayerId;
	start_area: SpaceId;
	start_exit: SpaceId;
	home: SpaceId;
}

export interface BoardGeometry {
	/** `[xmin, ymin, xmax, ymax]` — normalized board-local bounds. */
	bounds: [number, number, number, number];
	spaces: SpaceLayout[];
	slides: SlideLayout[];
	/** Indexed by `PlayerId`. */
	players: PlayerLayout[];
}

// ─── Helpers ──────────────────────────────────────────────────────────

export function spaceById(geom: BoardGeometry, id: SpaceId): SpaceLayout {
	for (const s of geom.spaces) {
		if (s.id === id) return s;
	}
	throw new Error(`unknown SpaceId: ${id}`);
}

/**
 * Convert a normalized `[-1, 1]` center to planar world-space `(x, z)`.
 * `y` (vertical) is left for the renderer to add. `scale` multiplies both
 * planar axes — default `2` puts the board in a `4×4` world square.
 */
export function toPlanar(center: [number, number], scale = 2.0): [number, number] {
	return [center[0] * scale, center[1] * scale];
}

export function isTrack(kind: SpaceKind): kind is { Track: number } {
	return 'Track' in kind;
}

export function isStartOf(kind: SpaceKind, player: PlayerId): boolean {
	return 'StartArea' in kind && kind.StartArea === player;
}

export function isSafetyOf(kind: SpaceKind, player: PlayerId): boolean {
	return 'Safety' in kind && kind.Safety[0] === player;
}

export function isHomeOf(kind: SpaceKind, player: PlayerId): boolean {
	return 'Home' in kind && kind.Home === player;
}
