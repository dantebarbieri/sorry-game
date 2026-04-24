// Where each of the four board sides "sits" around the board, in camera
// azimuth terms. Shared between the play route (camera rotation to the
// active seat) and the 3D renderer (positioning the active-turn card in
// front of the active player's edge), so both agree on which direction is
// Red / Blue / Yellow / Green.
//
// Azimuth values match what OrbitControls.getAzimuthalAngle returns after
// `edge`-view snapping: 0 → +Z, π/2 → +X, π → -Z, -π/2 → -X.
//
// | side | color  | azimuth |
// | ---- | ------ | ------- |
// |  0   | Red    |  π      |
// |  1   | Blue   |  π/2    |
// |  2   | Yellow |  0      |
// |  3   | Green  | -π/2    |
export function edgeAzimuthForSide(side: number): number {
	const map = [Math.PI, Math.PI / 2, 0, -Math.PI / 2];
	return map[side] ?? 0;
}

/**
 * World-space (x, z) position slightly inside the board's edge on the given
 * side — where a player is conceptually "seated." Used to place the
 * active-turn card in front of the active player. Board lives in the XZ
 * plane centered at the origin with `BOARD_SCALE = 2.0` (see renderer.ts),
 * so an `edgeDist` of ~1.55 sits inside the board's outer ring.
 *
 * Axis mapping mirrors the camera math in `renderer.applyCameraSpherical`:
 * the camera for side `s` sits at `(sin(az), _, cos(az)) * r`, so the
 * player's *seat* is on that same ray.
 */
export function edgePosition(
	side: number,
	edgeDist = 1.55
): { x: number; z: number } {
	const az = edgeAzimuthForSide(side);
	return {
		x: Math.sin(az) * edgeDist,
		z: Math.cos(az) * edgeDist
	};
}
