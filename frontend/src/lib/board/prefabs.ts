import * as THREE from 'three';
import type { BoardSkin } from './skins';

const TILE_SIZE = 0.19;
const TILE_HEIGHT = 0.018;
const START_RADIUS = 0.16;
const HOME_RADIUS = 0.12;
const SAFETY_INWARD = 0.15; // tighter than TILE_SIZE to fit the denser safety step
const DECK_WIDTH = 0.26;
const DECK_DEPTH = 0.36;
const DECK_HEIGHT = 0.065;
const DISCARD_HEIGHT = 0.025;

export function boardBase(skin: BoardSkin, size = 4.2): THREE.Mesh {
	const geom = new THREE.BoxGeometry(size, 0.06, size);
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(skin.palette.board),
		roughness: 0.85,
		metalness: 0.0
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = -0.03;
	mesh.receiveShadow = true;
	return mesh;
}

export function trackTile(skin: BoardSkin): THREE.Mesh {
	const geom = new THREE.BoxGeometry(TILE_SIZE, TILE_HEIGHT, TILE_SIZE);
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(skin.palette.trackTile),
		roughness: 0.75
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = TILE_HEIGHT / 2;
	mesh.receiveShadow = true;
	return mesh;
}

export function startDisk(skin: BoardSkin, player: number): THREE.Mesh {
	const geom = new THREE.CylinderGeometry(START_RADIUS, START_RADIUS, TILE_HEIGHT, 32);
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(skin.palette.players[player]),
		roughness: 0.6
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = TILE_HEIGHT / 2;
	mesh.receiveShadow = true;
	return mesh;
}

export function homePocket(skin: BoardSkin, player: number): THREE.Mesh {
	const geom = new THREE.CylinderGeometry(HOME_RADIUS, HOME_RADIUS * 1.1, TILE_HEIGHT * 1.4, 32);
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(skin.palette.players[player]),
		roughness: 0.5,
		metalness: 0.08
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = (TILE_HEIGHT * 1.4) / 2;
	mesh.receiveShadow = true;
	return mesh;
}

export function safetyChannelSegment(skin: BoardSkin, player: number): THREE.Mesh {
	const geom = new THREE.BoxGeometry(TILE_SIZE * 0.9, TILE_HEIGHT, SAFETY_INWARD);
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(skin.palette.players[player]),
		roughness: 0.7
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = TILE_HEIGHT / 2;
	mesh.receiveShadow = true;
	return mesh;
}

/** Face-down deck stack at the board center. Height implies card count. */
export function deckStack(skin: BoardSkin): THREE.Mesh {
	const geom = new THREE.BoxGeometry(DECK_WIDTH, DECK_HEIGHT, DECK_DEPTH);
	const deckColor = new THREE.Color(skin.palette.board).multiplyScalar(0.5);
	const mat = new THREE.MeshStandardMaterial({
		color: deckColor,
		roughness: 0.6,
		metalness: 0.05
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = DECK_HEIGHT / 2;
	mesh.castShadow = true;
	mesh.receiveShadow = true;
	return mesh;
}

/** Face-up discard pile — thinner than the deck, trackTile color as "card face". */
export function discardStack(skin: BoardSkin): THREE.Mesh {
	const geom = new THREE.BoxGeometry(DECK_WIDTH, DISCARD_HEIGHT, DECK_DEPTH);
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(skin.palette.trackTile),
		roughness: 0.72
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = DISCARD_HEIGHT / 2;
	mesh.castShadow = true;
	mesh.receiveShadow = true;
	return mesh;
}

/**
 * Slide visual: narrow colored bar running from head to end. The bar
 * extends back under the head triangle and forward under the end circle
 * so the body's four corners are hidden by the markers. The triangle and
 * circle rise from the board surface to (just above) the body's top, so
 * nothing hovers. Slight height offset on the markers prevents z-fighting
 * with the body's top face where they overlap.
 *
 * All coordinates in world space.
 */
export function slideRamp(
	skin: BoardSkin,
	owner: number,
	start: [number, number],
	end: [number, number]
): THREE.Group {
	const group = new THREE.Group();
	const dx = end[0] - start[0];
	const dz = end[1] - start[1];
	const length = Math.hypot(dx, dz);
	const dirX = dx / length;
	const dirZ = dz / length;
	const angle = Math.atan2(dx, dz);

	const ownerColor = new THREE.Color(skin.palette.players[owner]);
	const markerColor =
		skin.palette.markerShade === 'lighter'
			? ownerColor.clone().lerp(new THREE.Color('#ffffff'), 0.25)
			: ownerColor.clone().multiplyScalar(0.55);

	const bodyHeight = TILE_HEIGHT * 1.4;
	const bodyWidth = TILE_SIZE * 0.42;
	const bodyHalfW = bodyWidth / 2;

	// Marker shape constants.
	const triHalfW = TILE_SIZE * 0.28;
	const triHalfD = TILE_SIZE * 0.36;
	const endRadius = TILE_SIZE * 0.3;

	// Extend the body back/forward so its corners sit *inside* the marker
	// footprints. `backExt` stops just short of the triangle's base (ε avoids
	// coplanar back faces). `fwdExt` is bounded so body front corners stay
	// within the circle disk: sqrt(fwdExt² + halfW²) ≤ endRadius.
	const epsilon = 0.0005;
	const backExt = triHalfD - epsilon;
	const maxFwd = Math.sqrt(Math.max(0, endRadius * endRadius - bodyHalfW * bodyHalfW));
	const fwdExt = Math.max(0, maxFwd - epsilon);

	const bodyLen = length + backExt + fwdExt;
	const centerOffset = (fwdExt - backExt) / 2;
	const midX = (start[0] + end[0]) / 2 + centerOffset * dirX;
	const midZ = (start[1] + end[1]) / 2 + centerOffset * dirZ;

	const bodyGeom = new THREE.BoxGeometry(bodyWidth, bodyHeight, bodyLen);
	const bodyMat = new THREE.MeshStandardMaterial({
		color: ownerColor,
		roughness: 0.55,
		metalness: 0.05
	});
	const body = new THREE.Mesh(bodyGeom, bodyMat);
	body.position.set(midX, bodyHeight / 2, midZ);
	body.rotation.y = angle;
	body.receiveShadow = true;
	group.add(body);

	const markerMat = new THREE.MeshStandardMaterial({
		color: markerColor,
		roughness: 0.35,
		metalness: 0.15
	});
	// Markers reach slightly above the body so their top face isn't
	// coplanar with the body's top face (z-fight avoidance).
	const markerHeight = bodyHeight + 0.0015;

	// Triangle at head, base at y = 0, pointing toward end.
	const triShape = new THREE.Shape();
	triShape.moveTo(-triHalfW, -triHalfD);
	triShape.lineTo(triHalfW, -triHalfD);
	triShape.lineTo(0, triHalfD);
	triShape.closePath();
	const triGeom = new THREE.ExtrudeGeometry(triShape, {
		depth: markerHeight,
		bevelEnabled: false
	});
	// ExtrudeGeometry sits in x/y and extrudes along +z. Rotate so the
	// triangle lies flat on x/z; translate so it sits on y ∈ [0, height].
	triGeom.rotateX(Math.PI / 2);
	triGeom.translate(0, markerHeight, 0);
	const triangle = new THREE.Mesh(triGeom, markerMat);
	triangle.position.set(start[0], 0, start[1]);
	triangle.rotation.y = angle;
	triangle.castShadow = true;
	triangle.receiveShadow = true;
	group.add(triangle);

	// Cylinder at end, base at y = 0.
	const endGeom = new THREE.CylinderGeometry(endRadius, endRadius, markerHeight, 32);
	const endMarker = new THREE.Mesh(endGeom, markerMat);
	endMarker.position.set(end[0], markerHeight / 2, end[1]);
	endMarker.castShadow = true;
	endMarker.receiveShadow = true;
	group.add(endMarker);

	return group;
}
