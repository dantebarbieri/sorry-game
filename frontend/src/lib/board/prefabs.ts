import * as THREE from 'three';
import type { BoardSkin } from './skins';

const TILE_SIZE = 0.19;
const TILE_HEIGHT = 0.018;
const START_RADIUS = 0.16;
const HOME_RADIUS = 0.15;
const SAFETY_INWARD = 0.15; // tighter than TILE_SIZE to fit the denser safety step
const DECK_WIDTH = 0.26;
const DECK_DEPTH = 0.36;
// Per-card thickness — each card contributes the same vertical increment
// to both the deck and discard stacks. With StandardRules' 45-card deck
// that caps the deck at ~0.09 world units when full. Discards use the
// same rate so the relationship is intuitive: as one shrinks, the other
// grows at the same speed.
const CARD_THICKNESS = 0.002;
const MIN_STACK_HEIGHT = 0.006;
const PICKER_RADIUS = 0.11;
const HIGHLIGHT_INNER = 0.09;
const HIGHLIGHT_OUTER = 0.13;

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

function stackHeight(cards: number): number {
	if (cards <= 0) return 0;
	return Math.max(MIN_STACK_HEIGHT, cards * CARD_THICKNESS);
}

/** Face-down deck stack. Height scales linearly with remaining card count. */
export function deckStack(skin: BoardSkin, cards: number): THREE.Mesh {
	const height = stackHeight(cards);
	const geom = new THREE.BoxGeometry(DECK_WIDTH, height, DECK_DEPTH);
	const deckColor = new THREE.Color(skin.palette.board).multiplyScalar(0.5);
	const mat = new THREE.MeshStandardMaterial({
		color: deckColor,
		roughness: 0.6,
		metalness: 0.05
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = height / 2;
	mesh.castShadow = true;
	mesh.receiveShadow = true;
	return mesh;
}

/**
 * Invisible disk that catches pointer raycasts for a space. Rendered with
 * visible = false so it contributes nothing visually but still participates
 * in `Raycaster.intersectObjects`. Callers may pass a larger radius for
 * stacking spaces (Start / Home) so the full colored disk is clickable.
 */
export function spacePicker(radius = PICKER_RADIUS): THREE.Mesh {
	const geom = new THREE.CircleGeometry(radius, 24);
	geom.rotateX(-Math.PI / 2);
	const mat = new THREE.MeshBasicMaterial({ visible: false });
	const mesh = new THREE.Mesh(geom, mat);
	// Sit slightly above the board surface so raycasts hit the picker
	// before any underlying geometry.
	mesh.position.y = 0.01;
	return mesh;
}

/**
 * Flat ring decal shown on a legal destination square. Color matches the
 * moving pawn's owner (picked by the caller). Rendered slightly above any
 * tile mesh so it's always visible.
 */
export function destinationRing(color: THREE.ColorRepresentation): THREE.Mesh {
	const geom = new THREE.RingGeometry(HIGHLIGHT_INNER, HIGHLIGHT_OUTER, 32);
	geom.rotateX(-Math.PI / 2);
	const mat = new THREE.MeshBasicMaterial({
		color,
		transparent: true,
		opacity: 0.9,
		side: THREE.DoubleSide,
		depthWrite: false
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = 0.035;
	mesh.renderOrder = 10;
	return mesh;
}

/**
 * Halo disk drawn under a selected pawn to mark the current pick.
 * Visually similar to a destination ring but filled.
 */
/**
 * Filled disk used to mark the currently-focused destination (the one
 * `Enter` would commit under keyboard nav). Visually more prominent than
 * a ring so it reads as "primary" against the regular legal destinations.
 */
export function activeDestinationDisk(color: THREE.ColorRepresentation): THREE.Mesh {
	const geom = new THREE.CircleGeometry(HIGHLIGHT_OUTER * 1.02, 36);
	geom.rotateX(-Math.PI / 2);
	const mat = new THREE.MeshBasicMaterial({
		color,
		transparent: true,
		opacity: 0.7,
		side: THREE.DoubleSide,
		depthWrite: false
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = 0.036;
	mesh.renderOrder = 11;
	return mesh;
}

/**
 * Dimmer, distinct ring used for a locked Split-7 first-leg destination so
 * it reads as "already committed" rather than "pending your click."
 */
export function lockedDestinationRing(color: THREE.ColorRepresentation): THREE.Mesh {
	const geom = new THREE.RingGeometry(HIGHLIGHT_OUTER * 0.85, HIGHLIGHT_OUTER * 1.05, 32);
	geom.rotateX(-Math.PI / 2);
	const mat = new THREE.MeshBasicMaterial({
		color,
		transparent: true,
		opacity: 0.35,
		side: THREE.DoubleSide,
		depthWrite: false
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = 0.034;
	mesh.renderOrder = 8;
	return mesh;
}

export function selectionHalo(color: THREE.ColorRepresentation): THREE.Mesh {
	const geom = new THREE.CircleGeometry(HIGHLIGHT_OUTER, 32);
	geom.rotateX(-Math.PI / 2);
	const mat = new THREE.MeshBasicMaterial({
		color,
		transparent: true,
		opacity: 0.45,
		side: THREE.DoubleSide,
		depthWrite: false
	});
	const mesh = new THREE.Mesh(geom, mat);
	mesh.position.y = 0.033;
	mesh.renderOrder = 9;
	return mesh;
}

/**
 * Face-up discard pile. Height scales with discard size; if `topCardLabel`
 * is provided (e.g. "5", "Sorry!"), a small face-up canvas texture is
 * drawn on top showing the most recently discarded card.
 */
export function discardStack(
	skin: BoardSkin,
	cards: number,
	topCardLabel?: string | null,
	topCardColor?: string
): THREE.Group {
	const group = new THREE.Group();
	const height = stackHeight(cards);
	if (height > 0) {
		const box = new THREE.BoxGeometry(DECK_WIDTH, height, DECK_DEPTH);
		const boxMat = new THREE.MeshStandardMaterial({
			color: new THREE.Color(skin.palette.trackTile),
			roughness: 0.72
		});
		const boxMesh = new THREE.Mesh(box, boxMat);
		boxMesh.position.y = height / 2;
		boxMesh.castShadow = true;
		boxMesh.receiveShadow = true;
		group.add(boxMesh);
	}

	if (topCardLabel) {
		const face = cardFacePlane(topCardLabel, topCardColor ?? '#222222');
		face.position.y = height + 0.001;
		group.add(face);
	}
	return group;
}

/**
 * A face-down card back drawn on a thin plane. Matches `cardFacePlane`'s
 * size so flipping between back and face can animate without a size pop.
 * Used by the active-turn card during its "drawn from deck" flip.
 */
export function cardBackPlane(skin: BoardSkin): THREE.Mesh {
	const canvas = document.createElement('canvas');
	canvas.width = 256;
	canvas.height = 360;
	const ctx = canvas.getContext('2d');
	if (ctx) {
		const base = new THREE.Color(skin.palette.board).multiplyScalar(0.55);
		const baseCss = `#${base.getHexString()}`;
		const accent = new THREE.Color(skin.palette.board).multiplyScalar(0.35);
		const accentCss = `#${accent.getHexString()}`;
		ctx.fillStyle = baseCss;
		ctx.fillRect(0, 0, canvas.width, canvas.height);
		ctx.strokeStyle = accentCss;
		ctx.lineWidth = 8;
		ctx.strokeRect(4, 4, canvas.width - 8, canvas.height - 8);
		ctx.lineWidth = 14;
		ctx.beginPath();
		ctx.moveTo(30, 30);
		ctx.lineTo(canvas.width - 30, canvas.height - 30);
		ctx.moveTo(canvas.width - 30, 30);
		ctx.lineTo(30, canvas.height - 30);
		ctx.stroke();
	}
	const texture = new THREE.CanvasTexture(canvas);
	texture.colorSpace = THREE.SRGBColorSpace;
	texture.needsUpdate = true;
	const geom = new THREE.PlaneGeometry(DECK_WIDTH * 0.92, DECK_DEPTH * 0.92);
	geom.rotateX(-Math.PI / 2);
	const mat = new THREE.MeshBasicMaterial({ map: texture, transparent: false });
	return new THREE.Mesh(geom, mat);
}

/**
 * A face-up card decal drawn on a thin plane. Cream body, colored label,
 * matching border. Reused by the discard top-face and by the active-turn
 * card mesh so both share a visual vocabulary.
 *
 * Geometry is rotated so the face points up (+Y). Width/depth are sized to
 * match `DECK_WIDTH` / `DECK_DEPTH` so stacking the face directly on a
 * deck/discard box looks flush.
 */
export function cardFacePlane(label: string, color: string): THREE.Mesh {
	const canvas = document.createElement('canvas');
	canvas.width = 256;
	canvas.height = 360;
	const ctx = canvas.getContext('2d');
	if (ctx) {
		// Rounded-rect card body: cream fill, colored border matching the
		// active player. Transparent background outside the rounded rect
		// makes the corners round visually (rather than sit on a square
		// cream card).
		const pad = 10;
		const radius = 28;
		ctx.clearRect(0, 0, canvas.width, canvas.height);
		ctx.fillStyle = '#FBF6E7';
		ctx.strokeStyle = color;
		ctx.lineWidth = 12;
		ctx.lineJoin = 'round';
		ctx.beginPath();
		ctx.roundRect(pad, pad, canvas.width - 2 * pad, canvas.height - 2 * pad, radius);
		ctx.fill();
		ctx.stroke();

		ctx.fillStyle = color;
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		const isSorry = label === 'Sorry!';
		if (isSorry) {
			// Diagonal "Sorry!" runs bottom-left → top-right of the card.
			// Canvas y is inverted, so rotating by -60° in canvas = 60°
			// counterclockwise in math y-up.
			ctx.save();
			ctx.translate(canvas.width / 2, canvas.height / 2);
			ctx.rotate(-Math.PI / 3);
			ctx.font = 'bold 96px system-ui, sans-serif';
			ctx.fillText(label, 0, 0);
			ctx.restore();
		} else {
			ctx.font = `bold ${label.length > 2 ? 140 : 200}px system-ui, sans-serif`;
			ctx.fillText(label, canvas.width / 2, canvas.height / 2);
		}
	}
	const texture = new THREE.CanvasTexture(canvas);
	texture.colorSpace = THREE.SRGBColorSpace;
	texture.needsUpdate = true;
	const geom = new THREE.PlaneGeometry(DECK_WIDTH * 0.92, DECK_DEPTH * 0.92);
	geom.rotateX(-Math.PI / 2);
	// `alphaTest` keeps the rounded-corner transparent pixels from rendering
	// as opaque cream against the deck/discard box underneath.
	const mat = new THREE.MeshBasicMaterial({
		map: texture,
		transparent: true,
		alphaTest: 0.05
	});
	return new THREE.Mesh(geom, mat);
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
