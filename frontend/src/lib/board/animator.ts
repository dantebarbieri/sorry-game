import * as THREE from 'three';

import type { BoardGeometry, PlayerId, SpaceId } from './geometry';
import { toPlanar } from './geometry';
import { hopPath } from './paths';
import { easeInOutCubic, easeOutQuad, tween, wait } from './timeline';
import type { BumpEvent, PlayRecord, SlideEvent } from './actions';

const HOP_MS = 120;
const HOP_HEIGHT = 0.035;
const SLIDE_MS_PER_SPACE = 55;
const ARC_MS = 380;
const ARC_HEIGHT = 0.12;
// Pass has no visual motion, but we still want the card chip to "dwell" on
// the passing player's card long enough for the viewer to register it
// before the queue advances to the next step's animation.
const PASS_DWELL_MS = 500;

interface AnimatorConfig {
	geometry: BoardGeometry;
	scale: number;
	pawnY: number;
	getMesh: (player: PlayerId, pawn: number) => THREE.Mesh | null;
}

/**
 * Sequences three.js mesh transforms for a single engine `Action::Play`.
 * Queue semantics: calls to `enqueue` are serialized via a FIFO promise
 * chain so consecutive bot steps play back in order even if the user
 * hammers the button. Reduce-motion short-circuits every tween to a
 * zero-duration snap.
 */
export class Animator {
	private queue: Promise<void> = Promise.resolve();
	private reducedMotion = false;
	private cfg: AnimatorConfig;
	// Monotonically-increasing generation. `cancel()` bumps it; every
	// already-enqueued task captured its own `myGen` at enqueue time and
	// compares against `this.cancelGen` at each await boundary, bailing
	// out (without firing `onDone`) once the generations diverge.
	private cancelGen = 0;

	constructor(cfg: AnimatorConfig) {
		this.cfg = cfg;
	}

	setReducedMotion(v: boolean): void {
		this.reducedMotion = v;
	}

	setGeometry(geometry: BoardGeometry): void {
		this.cfg.geometry = geometry;
	}

	/**
	 * Invalidate every already-enqueued task. In-flight tweens finish their
	 * current frame (cheap — <16ms) but skip everything after; pending
	 * tasks abort before firing `onStart`, and no `onDone` ever runs (so
	 * a stale post-animation snap can't clobber a fresh state).
	 */
	cancel(): void {
		this.cancelGen++;
	}

	enqueue(
		prevPositions: SpaceId[][],
		record: PlayRecord,
		currentPlayer: PlayerId,
		handlers: { onStart?: () => void; onDone?: () => void } = {}
	): Promise<void> {
		const myGen = this.cancelGen;
		const isStale = () => myGen !== this.cancelGen;
		const task = async () => {
			if (isStale()) return;
			try {
				handlers.onStart?.();
				await this.playAction(prevPositions, record, currentPlayer, isStale);
			} finally {
				if (!isStale()) handlers.onDone?.();
			}
		};
		this.queue = this.queue
			.then(task)
			.catch((err) => console.error('[animator] task failed:', err));
		return this.queue;
	}

	// ─── Core dispatch ──────────────────────────────────────────────

	private async playAction(
		prev: SpaceId[][],
		record: PlayRecord,
		player: PlayerId,
		isStale: () => boolean
	): Promise<void> {
		if (this.reducedMotion || isStale()) return;
		const mv = record.mv;
		switch (mv.type) {
			case 'Advance':
			case 'Retreat': {
				const mesh = this.cfg.getMesh(player, mv.pawn);
				if (!mesh) break;
				const from = prev[player]?.[mv.pawn];
				if (from == null) break;
				const fromLayout = this.cfg.geometry.spaces.find((s) => s.id === from);
				const fromKind = fromLayout?.kind;
				if (fromKind && ('StartArea' in fromKind || 'Home' in fromKind)) {
					console.warn(
						`[animator] ${mv.type} from ${JSON.stringify(fromKind)} — skipping animation`
					);
					break;
				}
				const dir = mv.type === 'Advance' ? 'forward' : 'backward';
				const path = hopPath(this.cfg.geometry, player, from, mv.to, dir);
				if (path && path.length > 1) {
					await this.hopAlong(mesh, path.slice(1), isStale);
				} else {
					console.warn(
						`[animator] no ${dir} path ${from} → ${mv.to} for P${player}; skipping animation`
					);
				}
				break;
			}
			case 'StartPawn': {
				const mesh = this.cfg.getMesh(player, mv.pawn);
				if (!mesh) break;
				const startExit = this.cfg.geometry.players.find(
					(p) => p.player === player
				)?.start_exit;
				if (startExit == null) break;
				await this.hopToWorld(mesh, this.worldPos(startExit));
				if (isStale()) break;
				if (mv.to !== startExit) {
					const path = hopPath(this.cfg.geometry, player, startExit, mv.to, 'forward');
					if (path && path.length > 1) await this.hopAlong(mesh, path.slice(1), isStale);
				}
				break;
			}
			case 'Sorry': {
				const myMesh = this.cfg.getMesh(player, mv.my_pawn);
				const theirMesh = this.cfg.getMesh(mv.their_player, mv.their_pawn);
				const theirStart = this.startArea(mv.their_player);
				const tasks: Promise<void>[] = [];
				if (myMesh) tasks.push(this.arcToSpace(myMesh, mv.to));
				if (theirMesh && theirStart != null)
					tasks.push(this.arcToSpace(theirMesh, theirStart));
				await Promise.all(tasks);
				break;
			}
			case 'SwapEleven': {
				const myMesh = this.cfg.getMesh(player, mv.my_pawn);
				const theirMesh = this.cfg.getMesh(mv.their_player, mv.their_pawn);
				if (myMesh && theirMesh) {
					const myFrom = myMesh.position.clone();
					const theirFrom = theirMesh.position.clone();
					await Promise.all([
						this.arcToWorld(myMesh, theirFrom),
						this.arcToWorld(theirMesh, myFrom)
					]);
				}
				break;
			}
			case 'SplitSeven': {
				const m1 = this.cfg.getMesh(player, mv.first.pawn);
				if (m1) {
					const from1 = prev[player]?.[mv.first.pawn];
					if (from1 != null) {
						const path1 = hopPath(this.cfg.geometry, player, from1, mv.first.to, 'forward');
						if (path1 && path1.length > 1)
							await this.hopAlong(m1, path1.slice(1), isStale);
					}
				}
				if (isStale()) break;
				const m2 = this.cfg.getMesh(player, mv.second.pawn);
				if (m2) {
					const from2 = prev[player]?.[mv.second.pawn];
					if (from2 != null) {
						const path2 = hopPath(this.cfg.geometry, player, from2, mv.second.to, 'forward');
						if (path2 && path2.length > 1)
							await this.hopAlong(m2, path2.slice(1), isStale);
					}
				}
				break;
			}
			case 'Pass':
				await wait(PASS_DWELL_MS);
				break;
		}

		if (isStale()) return;

		const slideHandled = new Set<string>();
		for (const slide of record.slides) {
			if (isStale()) return;
			const mesh = this.cfg.getMesh(slide.player, slide.pawn);
			if (!mesh) continue;
			const overlappingBumps = record.bumps.filter((b) => slide.path.includes(b.from));
			for (const b of overlappingBumps) slideHandled.add(bumpKey(b));
			await Promise.all([
				this.slideAlong(mesh, slide.path, isStale),
				...overlappingBumps.map((b) => this.bumpToStart(b))
			]);
		}

		if (isStale()) return;

		const otherBumps = record.bumps.filter((b) => !slideHandled.has(bumpKey(b)));
		await Promise.all(otherBumps.map((b) => this.bumpToStart(b)));
	}

	// ─── Motion primitives ──────────────────────────────────────────

	private async hopAlong(
		mesh: THREE.Mesh,
		spaceIds: SpaceId[],
		isStale: () => boolean
	): Promise<void> {
		for (const id of spaceIds) {
			if (isStale()) return;
			await this.hopToWorld(mesh, this.worldPos(id));
		}
	}

	private async hopToWorld(mesh: THREE.Mesh, target: THREE.Vector3): Promise<void> {
		const from = mesh.position.clone();
		await tween(
			HOP_MS,
			(t) => {
				mesh.position.lerpVectors(from, target, t);
				mesh.position.y += Math.sin(t * Math.PI) * HOP_HEIGHT;
			},
			easeInOutCubic
		);
		mesh.position.copy(target);
	}

	private async slideAlong(
		mesh: THREE.Mesh,
		path: SpaceId[],
		isStale: () => boolean
	): Promise<void> {
		for (const id of path) {
			if (isStale()) return;
			const target = this.worldPos(id);
			const from = mesh.position.clone();
			await tween(
				SLIDE_MS_PER_SPACE,
				(t) => mesh.position.lerpVectors(from, target, t),
				easeOutQuad
			);
			mesh.position.copy(target);
		}
	}

	private arcToSpace(mesh: THREE.Mesh, spaceId: SpaceId): Promise<void> {
		return this.arcToWorld(mesh, this.worldPos(spaceId));
	}

	private async arcToWorld(mesh: THREE.Mesh, target: THREE.Vector3): Promise<void> {
		const from = mesh.position.clone();
		await tween(
			ARC_MS,
			(t) => {
				mesh.position.lerpVectors(from, target, t);
				mesh.position.y += Math.sin(t * Math.PI) * ARC_HEIGHT;
			},
			easeInOutCubic
		);
		mesh.position.copy(target);
	}

	private bumpToStart(b: BumpEvent): Promise<void> {
		const mesh = this.cfg.getMesh(b.player, b.pawn);
		if (!mesh) return Promise.resolve();
		return this.arcToSpace(mesh, b.to);
	}

	// ─── Helpers ───────────────────────────────────────────────────

	private worldPos(spaceId: SpaceId): THREE.Vector3 {
		const layout = this.cfg.geometry.spaces.find((s) => s.id === spaceId);
		if (!layout) return new THREE.Vector3();
		const [x, z] = toPlanar(layout.center, this.cfg.scale);
		return new THREE.Vector3(x, this.cfg.pawnY, z);
	}

	private startArea(player: PlayerId): SpaceId | undefined {
		return this.cfg.geometry.players.find((p) => p.player === player)?.start_area;
	}
}

function bumpKey(b: BumpEvent): string {
	return `${b.player}:${b.pawn}:${b.from}`;
}

export type { SlideEvent };
