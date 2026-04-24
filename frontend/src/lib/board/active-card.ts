// The "active turn card" — a 3D card mesh that represents the card the
// current player just drew face-up. Sits on top of the deck face-down,
// flips over, and flies to the active player's edge of the board; when
// the play commits it flies to the discard pile. Lifecycle is driven by
// the renderer's prev→next state diff.
//
// Deliberately independent of `Animator` (which sequences pawn motion).
// The card's motion is its own track: it starts from the deck at turn
// begin and arrives at the discard on commit, alongside the pawn move
// rather than after it.
//
// Queue semantics mirror `Animator`: operations are FIFO through a
// promise chain so a commit-then-draw pair plays in order. `cancel()`
// bumps `cancelGen`; each queued task captures its own gen at enqueue
// time and bails out at await boundaries when the gens diverge. Used on
// New Game / replay to avoid a stale animation clobbering fresh state.

import * as THREE from 'three';

import { cardBackPlane, cardFacePlane } from './prefabs';
import type { BoardSkin } from './skins';
import { easeInOutCubic, tween } from './timeline';

// Match the deck/discard placement in `renderer.rebuildDeckDiscard`. If
// those offsets change, update here too — the activeCard needs to start
// on top of the deck and land on top of the discard.
const DECK_CENTER_X = -0.18;
const DISCARD_CENTER_X = 0.18;
const STACK_HOVER_Y = 0.04; // slightly above a full deck/discard stack
const EDGE_Y = 0.06; // clear of the board surface when seated at the edge

const DRAW_FLIGHT_MS = 520;
const COMMIT_FLIGHT_MS = 440;
const FLIGHT_ARC_HEIGHT = 0.22;
// The card is already facing up by ~70 % through the flight so the
// landing on the player's edge looks settled rather than still-flipping.
const FLIP_RATIO = 0.7;

export interface EdgeSeat {
	side: number;
	x: number;
	z: number;
	/** Y-axis rotation that points the card's label "up" toward the player
	 *  sitting at this edge — so e.g. Red (across the board) reads text
	 *  right-side-up from their camera. */
	rotY: number;
}

export class ActiveCardAnim {
	private readonly group = new THREE.Group();
	private front: THREE.Mesh | null = null;
	private back: THREE.Mesh;
	private skin: BoardSkin;
	private reducedMotion = false;
	private queue: Promise<void> = Promise.resolve();
	private cancelGen = 0;
	private scene: THREE.Scene;

	constructor(scene: THREE.Scene, skin: BoardSkin) {
		this.scene = scene;
		this.skin = skin;
		this.back = cardBackPlane(skin);
		// `cardBackPlane` returns a plane whose face points +Y. Flip it so
		// its face points -Y, then sit it just under the group origin —
		// paired with a face-up front plane just above origin, the two form
		// a two-sided card. When group.rotation.x = 0 the front is visible
		// from above; at group.rotation.x = π the back is visible.
		this.back.rotateX(Math.PI);
		this.back.position.y = -0.001;
		this.group.add(this.back);
		this.group.visible = false;
		this.scene.add(this.group);
	}

	setSkin(skin: BoardSkin): void {
		if (this.skin.id === skin.id) return;
		this.skin = skin;
		this.group.remove(this.back);
		disposeMesh(this.back);
		this.back = cardBackPlane(skin);
		this.back.rotateX(Math.PI);
		this.back.position.y = -0.001;
		this.group.add(this.back);
	}

	setReducedMotion(v: boolean): void {
		this.reducedMotion = v;
	}

	/** Invalidate every already-enqueued task. In-flight tweens finish their
	 *  current frame and bail; pending tasks abort before mutating state. */
	cancel(): void {
		this.cancelGen++;
		this.queue = Promise.resolve();
	}

	hide(): void {
		this.cancel();
		this.group.visible = false;
	}

	/**
	 * Spawn at the deck face-down, flip face-up, and fly to the active
	 * player's edge. `accentColor` tints the card's label / border.
	 * The card also rotates around the Y axis as it flies so its label
	 * settles right-side-up for whoever is seated at `edge`. `replaceFront`
	 * runs inside the queue so a prior `flyToDiscard` still sees the
	 * *previous* card's face while it's animating.
	 */
	drawToEdge(edge: EdgeSeat, label: string, accentColor: string): void {
		this.enqueue(async (isStale) => {
			if (isStale()) return;
			this.replaceFront(label, accentColor);
			this.group.position.set(DECK_CENTER_X, STACK_HOVER_Y, 0);
			this.group.rotation.set(Math.PI, 0, 0);
			this.group.visible = true;

			if (this.reducedMotion) {
				this.group.position.set(edge.x, EDGE_Y, edge.z);
				this.group.rotation.set(0, edge.rotY, 0);
				return;
			}

			const start = this.group.position.clone();
			const end = new THREE.Vector3(edge.x, EDGE_Y, edge.z);
			const startRotY = 0;
			const endRotY = edge.rotY;

			await tween(
				DRAW_FLIGHT_MS,
				(t) => {
					if (isStale()) return;
					this.group.position.lerpVectors(start, end, t);
					this.group.position.y += Math.sin(t * Math.PI) * FLIGHT_ARC_HEIGHT;
					const flipT = Math.min(1, t / FLIP_RATIO);
					this.group.rotation.x = Math.PI * (1 - flipT);
					this.group.rotation.y = startRotY + (endRotY - startRotY) * t;
				},
				easeInOutCubic
			);
			if (isStale()) return;
			this.group.position.copy(end);
			this.group.rotation.set(0, edge.rotY, 0);
		});
	}

	/**
	 * Fly the current face-up card from its current position (presumably an
	 * edge seat) onto the discard pile, then hide. Called when the active
	 * player commits their move.
	 *
	 * `onStart`/`onDone` fire on the flight boundaries (after any queued
	 * prior op completes) so the caller can synchronise the discard-pile
	 * top-face decal with the animation — suppress it while the card is
	 * airborne, reveal it once it lands.
	 */
	flyToDiscard(callbacks: {
		onStart?: () => void;
		onDone?: () => void;
	} = {}): void {
		this.enqueue(async (isStale) => {
			if (isStale() || !this.group.visible) return;
			callbacks.onStart?.();

			if (this.reducedMotion) {
				this.group.visible = false;
				callbacks.onDone?.();
				return;
			}

			const start = this.group.position.clone();
			const end = new THREE.Vector3(DISCARD_CENTER_X, STACK_HOVER_Y, 0);
			const startRotY = this.group.rotation.y;

			await tween(
				COMMIT_FLIGHT_MS,
				(t) => {
					if (isStale()) return;
					this.group.position.lerpVectors(start, end, t);
					this.group.position.y += Math.sin(t * Math.PI) * (FLIGHT_ARC_HEIGHT * 0.7);
					// Unwind the seat's Y rotation so the card lies flat
					// when it merges into the discard.
					this.group.rotation.y = startRotY * (1 - t);
				},
				easeInOutCubic
			);
			if (isStale()) return;
			this.group.visible = false;
			this.group.rotation.set(0, 0, 0);
			callbacks.onDone?.();
		});
	}

	dispose(): void {
		this.cancel();
		if (this.front) disposeMesh(this.front);
		disposeMesh(this.back);
		this.scene.remove(this.group);
	}

	private enqueue(op: (isStale: () => boolean) => Promise<void>): void {
		const myGen = this.cancelGen;
		const isStale = () => myGen !== this.cancelGen;
		this.queue = this.queue
			.then(() => op(isStale))
			.catch((err) => console.error('[ActiveCard] op failed:', err));
	}

	private replaceFront(label: string, accentColor: string): void {
		if (this.front) {
			this.group.remove(this.front);
			disposeMesh(this.front);
		}
		this.front = cardFacePlane(label, accentColor);
		this.front.position.y = 0.001;
		this.group.add(this.front);
	}
}

function disposeMesh(mesh: THREE.Mesh): void {
	mesh.geometry.dispose();
	const mat = mesh.material as THREE.MeshBasicMaterial;
	if (mat.map) mat.map.dispose();
	mat.dispose();
}
