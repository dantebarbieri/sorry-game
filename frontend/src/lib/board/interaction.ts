import * as THREE from 'three';

import type { SpaceId } from './geometry';

/** What the raycaster picked under the pointer. */
export type PickHit =
	| { kind: 'pawn'; player: number; pawn: number }
	| { kind: 'space'; spaceId: SpaceId };

/** Targets the raycaster should test, provided by the renderer each pick. */
export interface PickTargets {
	pawns: THREE.Object3D[];
	spaces: THREE.Object3D[];
}

/**
 * Owns `pointerdown` listening on the canvas and turns it into a high-level
 * pick event. Pawn hits take priority over space hits (pawns sit on top of
 * the picker disks, so the same click would otherwise ambiguously match
 * both). If nothing picks, the callback fires with `null` so the caller
 * can clear any pending selection.
 */
export class Interaction {
	private readonly raycaster = new THREE.Raycaster();
	private readonly pointer = new THREE.Vector2();
	private attached = false;
	private hoverCallback: ((hit: PickHit | null) => void) | null = null;

	constructor(
		private readonly canvas: HTMLCanvasElement,
		private readonly camera: THREE.Camera,
		private readonly getTargets: () => PickTargets,
		private readonly onPick: (hit: PickHit | null) => void
	) {}

	setHoverCallback(cb: ((hit: PickHit | null) => void) | null): void {
		this.hoverCallback = cb;
	}

	attach(): void {
		if (this.attached) return;
		this.canvas.addEventListener('pointerdown', this.handlePointerDown);
		this.canvas.addEventListener('pointermove', this.handlePointerMove);
		this.canvas.addEventListener('pointerleave', this.handlePointerLeave);
		this.attached = true;
	}

	detach(): void {
		if (!this.attached) return;
		this.canvas.removeEventListener('pointerdown', this.handlePointerDown);
		this.canvas.removeEventListener('pointermove', this.handlePointerMove);
		this.canvas.removeEventListener('pointerleave', this.handlePointerLeave);
		this.attached = false;
	}

	private raycastHit(clientX: number, clientY: number): PickHit | null {
		const rect = this.canvas.getBoundingClientRect();
		this.pointer.x = ((clientX - rect.left) / rect.width) * 2 - 1;
		this.pointer.y = -((clientY - rect.top) / rect.height) * 2 + 1;
		this.raycaster.setFromCamera(this.pointer, this.camera);

		const { pawns, spaces } = this.getTargets();

		const pawnHits = this.raycaster.intersectObjects(pawns, true);
		for (const hit of pawnHits) {
			const data = extractPawnData(hit.object);
			if (data) return { kind: 'pawn', ...data };
		}

		const spaceHits = this.raycaster.intersectObjects(spaces, false);
		for (const hit of spaceHits) {
			const data = extractSpaceData(hit.object);
			if (data !== null) return { kind: 'space', spaceId: data };
		}
		return null;
	}

	private handlePointerMove = (event: PointerEvent): void => {
		if (!this.hoverCallback) return;
		this.hoverCallback(this.raycastHit(event.clientX, event.clientY));
	};

	private handlePointerLeave = (): void => {
		this.hoverCallback?.(null);
	};

	private handlePointerDown = (event: PointerEvent): void => {
		// Only primary-button clicks should count as picks. OrbitControls
		// consumes drags on any button, but a drag starts with a pointerdown,
		// so we use a small movement threshold on pointerup? For now we just
		// treat the initial down as a pick — OrbitControls dampening handles
		// the delay until pointerup, and the downstream effect of a picked
		// pawn/space on a drag-start is visually tolerable.
		if (event.button !== 0) return;
		this.onPick(this.raycastHit(event.clientX, event.clientY));
	};
}

function extractPawnData(
	obj: THREE.Object3D
): { player: number; pawn: number } | null {
	let cur: THREE.Object3D | null = obj;
	while (cur) {
		const data = cur.userData as { player?: number; pawn?: number };
		if (data.player !== undefined && data.pawn !== undefined) {
			return { player: data.player, pawn: data.pawn };
		}
		cur = cur.parent;
	}
	return null;
}

function extractSpaceData(obj: THREE.Object3D): SpaceId | null {
	const data = obj.userData as { spaceId?: SpaceId };
	return data.spaceId ?? null;
}
