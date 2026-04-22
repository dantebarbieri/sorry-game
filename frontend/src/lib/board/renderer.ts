import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';

import type { BoardGeometry } from './geometry';
import { toPlanar, isTrack } from './geometry';
import type { BoardSkin } from './skins';
import type { GameStateView } from './state';
import type { PlayRecord } from './actions';
import { Animator } from './animator';
import {
	boardBase,
	deckStack,
	discardStack,
	homePocket,
	safetyChannelSegment,
	slideRamp,
	startDisk,
	trackTile
} from './prefabs';
import { loadPieceGeometry, makePiece } from './assets';

const BOARD_SCALE = 2.0; // maps [-1, 1] to world [-2, 2]
const PAWNS_PER_PLAYER = 4;
const PAWN_STACK_RADIUS = 0.055;
const PAWN_Y = 0.02;

export type CameraView = 'edge' | 'corner' | 'top';

// Initial camera position — an edge view at a comfortable tilt. The smart-
// snap logic in `setCameraView` preserves the user's current radius and
// polar when the preset change only needs azimuth adjustment.
const INITIAL_CAMERA: [number, number, number] = [0, 4.5, 3.0];
const DEFAULT_TILT = 0.19 * Math.PI; // polar when coming from top-down
const MIN_VISIBLE_TILT = 0.05 * Math.PI; // below this we treat the view as top-down
const TOP_DOWN_POLAR = 0.001; // tiny offset so the up-vector isn't anti-parallel

function snapToNearest(value: number, step: number, offset: number): number {
	return Math.round((value - offset) / step) * step + offset;
}

/**
 * Imperative three.js controller owning scene, camera, lights, board
 * prefabs, and pawn meshes. State-to-scene snapping in Phase 4; animation
 * (Phase 5) and interaction (Phase 6) layer on top.
 */
export class BoardRenderer {
	private readonly scene = new THREE.Scene();
	private readonly camera: THREE.PerspectiveCamera;
	private readonly renderer: THREE.WebGLRenderer;
	private readonly controls: OrbitControls;
	private readonly boardStaticGroup = new THREE.Group();
	private readonly piecesGroup = new THREE.Group();
	private readonly geometry: BoardGeometry;
	private skin: BoardSkin;
	private rafId: number | null = null;
	private readonly resizeObserver: ResizeObserver;
	private readonly visibilityHandler: () => void;

	private pawnGeometry: THREE.BufferGeometry | null = null;
	private pawnMeshes: THREE.Mesh[][] = [];
	private pendingState: GameStateView | null = null;
	private pendingStep: { record: PlayRecord; player: number } | null = null;
	private previousState: GameStateView | null = null;
	private pieceLoadError: Error | null = null;
	private userOrbitCallback: (() => void) | null = null;
	private animator: Animator;

	constructor(canvas: HTMLCanvasElement, geometry: BoardGeometry, skin: BoardSkin) {
		this.geometry = geometry;
		this.skin = skin;
		this.animator = new Animator({
			geometry,
			scale: BOARD_SCALE,
			pawnY: PAWN_Y,
			getMesh: (player, pawn) => this.pawnMeshes[player]?.[pawn] ?? null
		});

		this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
		this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		this.renderer.shadowMap.enabled = true;
		this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;

		const { width, height } = canvas.getBoundingClientRect();
		const aspect = width / Math.max(1, height);
		this.camera = new THREE.PerspectiveCamera(45, aspect, 0.1, 100);
		this.camera.position.set(...INITIAL_CAMERA);
		this.camera.lookAt(0, 0, 0);

		this.controls = new OrbitControls(this.camera, canvas);
		this.controls.addEventListener('start', () => {
			this.userOrbitCallback?.();
		});
		this.controls.enablePan = false;
		this.controls.enableDamping = true;
		this.controls.dampingFactor = 0.08;
		this.controls.autoRotate = false;
		// Allow straight-down (polar 0) so the "Top-down" preset is reachable.
		this.controls.minPolarAngle = 0;
		this.controls.maxPolarAngle = 0.46 * Math.PI;
		this.controls.minDistance = 3;
		this.controls.maxDistance = 8;

		this.scene.background = new THREE.Color(skin.palette.background);

		this.scene.add(new THREE.AmbientLight(0xffffff, 0.55));
		const key = new THREE.DirectionalLight(0xffffff, 1.0);
		key.position.set(3, 6, 2);
		key.castShadow = true;
		const mobile = window.matchMedia('(max-width: 768px)').matches;
		const shadowMap = mobile ? 512 : 1024;
		key.shadow.mapSize.set(shadowMap, shadowMap);
		key.shadow.camera.left = -3;
		key.shadow.camera.right = 3;
		key.shadow.camera.top = 3;
		key.shadow.camera.bottom = -3;
		key.shadow.camera.near = 0.5;
		key.shadow.camera.far = 20;
		this.scene.add(key);

		this.scene.add(this.boardStaticGroup);
		this.scene.add(this.piecesGroup);
		this.buildStatic();

		this.resizeObserver = new ResizeObserver(() => this.onResize());
		this.resizeObserver.observe(canvas);
		this.onResize();

		this.visibilityHandler = () => {
			if (document.hidden) this.stopLoop();
			else this.startLoop();
		};
		document.addEventListener('visibilitychange', this.visibilityHandler);
		this.startLoop();

		// Fire-and-forget piece load — state-snap runs once it resolves.
		loadPieceGeometry().then(
			(geom) => {
				this.pawnGeometry = geom;
				if (this.pendingState) {
					const state = this.pendingState;
					const step = this.pendingStep;
					this.pendingState = null;
					this.pendingStep = null;
					this.setState(state, step?.record, step?.player);
				}
			},
			(err) => {
				this.pieceLoadError = err instanceof Error ? err : new Error(String(err));
				console.error('[BoardRenderer] failed to load piece GLB:', this.pieceLoadError);
			}
		);
	}

	setSkin(skin: BoardSkin): void {
		if (this.skin.id === skin.id) return;
		this.skin = skin;
		this.scene.background = new THREE.Color(skin.palette.background);
		this.disposeGroup(this.boardStaticGroup);
		this.buildStatic();
		this.retintPawns();
	}

	setReducedMotion(v: boolean): void {
		this.animator.setReducedMotion(v);
	}

	/**
	 * Snap the camera to the preset VIEW TYPE nearest its current position,
	 * preserving radius and (when possible) polar. "Edge" snaps azimuth to
	 * the closest multiple of π/2; "corner" to the closest π/4 + k·π/2;
	 * "top" only changes polar.
	 */
	setCameraView(mode: CameraView): void {
		const azimuth = this.controls.getAzimuthalAngle();
		const polar = this.controls.getPolarAngle();
		const radius = this.camera.position.distanceTo(this.controls.target);

		let targetAzimuth = azimuth;
		let targetPolar = polar;

		switch (mode) {
			case 'edge':
				targetAzimuth = snapToNearest(azimuth, Math.PI / 2, 0);
				if (polar < MIN_VISIBLE_TILT) targetPolar = DEFAULT_TILT;
				break;
			case 'corner':
				targetAzimuth = snapToNearest(azimuth, Math.PI / 2, Math.PI / 4);
				if (polar < MIN_VISIBLE_TILT) targetPolar = DEFAULT_TILT;
				break;
			case 'top':
				targetPolar = TOP_DOWN_POLAR;
				break;
		}

		const target = this.controls.target;
		const sinP = Math.sin(targetPolar);
		const cosP = Math.cos(targetPolar);
		this.camera.position.set(
			target.x + radius * sinP * Math.sin(targetAzimuth),
			target.y + radius * cosP,
			target.z + radius * sinP * Math.cos(targetAzimuth)
		);
		this.controls.update();
	}

	setUserOrbitCallback(cb: (() => void) | null): void {
		this.userOrbitCallback = cb;
	}

	/**
	 * Apply a new game state. If a `record` + `player` are provided AND we
	 * have a previous state to diff from, the transition is animated via
	 * the animator. Otherwise pawns snap to the new positions (used on
	 * initial load, new-game reset, and reduced-motion mode). Optional
	 * callbacks fire when *this* animation actually starts / ends — useful
	 * for keeping HUD chrome in sync with the currently-playing move when
	 * multiple steps are queued.
	 */
	setState(
		state: GameStateView,
		record?: PlayRecord,
		player?: number,
		callbacks?: { onStart?: () => void; onDone?: () => void }
	): void {
		if (!this.pawnGeometry) {
			this.pendingState = state;
			this.pendingStep = record != null && player != null ? { record, player } : null;
			return;
		}
		this.ensurePawns(state.num_players);
		const prev = this.previousState;
		if (record && prev && player != null) {
			this.animator.enqueue(prev.pawn_positions, record, player, {
				onStart: callbacks?.onStart,
				onDone: () => {
					// Settle meshes into correct stacked positions after animation.
					this.snapPawns(state);
					callbacks?.onDone?.();
				}
			});
		} else {
			// No animation requested — this is an initial load, New Game, or
			// similar discontinuous state change. Invalidate any pending
			// animations so their post-animation snap can't clobber us with
			// a stale state.
			this.animator.cancel();
			this.snapPawns(state);
		}
		this.previousState = state;
	}

	dispose(): void {
		this.stopLoop();
		this.resizeObserver.disconnect();
		document.removeEventListener('visibilitychange', this.visibilityHandler);
		this.controls.dispose();
		this.disposeGroup(this.boardStaticGroup);
		this.disposeGroup(this.piecesGroup);
		this.pawnMeshes = [];
		this.renderer.dispose();
	}

	// ─── Internal ──────────────────────────────────────────────────

	private buildStatic(): void {
		this.boardStaticGroup.add(boardBase(this.skin));

		for (const layout of this.geometry.spaces) {
			const mesh = this.meshForSpace(layout.kind);
			if (!mesh) continue;
			const [x, z] = toPlanar(layout.center, BOARD_SCALE);
			mesh.position.x = x;
			mesh.position.z = z;
			mesh.rotation.y = -(layout.tangent_deg * Math.PI) / 180;
			this.boardStaticGroup.add(mesh);
		}

		for (const slide of this.geometry.slides) {
			const head = this.geometry.spaces.find((s) => s.id === slide.head);
			const end = this.geometry.spaces.find((s) => s.id === slide.end);
			if (!head || !end) continue;
			const [sx, sz] = toPlanar(head.center, BOARD_SCALE);
			const [ex, ez] = toPlanar(end.center, BOARD_SCALE);
			this.boardStaticGroup.add(slideRamp(this.skin, slide.owner, [sx, sz], [ex, ez]));
		}

		// Deck + discard placeholders at the board center. The engine doesn't
		// currently publish their positions, so they're hard-coded here —
		// revisit once the HUD / hand rendering session wants to coordinate.
		const deck = deckStack(this.skin);
		deck.position.x = -0.18;
		deck.position.z = 0;
		this.boardStaticGroup.add(deck);

		const discard = discardStack(this.skin);
		discard.position.x = 0.18;
		discard.position.z = 0;
		this.boardStaticGroup.add(discard);
	}

	private meshForSpace(
		kind: BoardGeometry['spaces'][number]['kind']
	): THREE.Mesh | null {
		if (isTrack(kind)) return trackTile(this.skin);
		if ('StartArea' in kind) return startDisk(this.skin, kind.StartArea);
		if ('Home' in kind) return homePocket(this.skin, kind.Home);
		if ('Safety' in kind) return safetyChannelSegment(this.skin, kind.Safety[0]);
		return null;
	}

	private ensurePawns(numPlayers: number): void {
		if (!this.pawnGeometry) return;
		while (this.pawnMeshes.length < numPlayers) {
			const p = this.pawnMeshes.length;
			const color = this.skin.palette.players[p] ?? '#888888';
			const row: THREE.Mesh[] = [];
			for (let k = 0; k < PAWNS_PER_PLAYER; k++) {
				const mesh = makePiece(this.pawnGeometry, color);
				this.piecesGroup.add(mesh);
				row.push(mesh);
			}
			this.pawnMeshes.push(row);
		}
		// Hide unused player slots (e.g. 2- or 3-player games on a 4-color board).
		for (let p = 0; p < this.pawnMeshes.length; p++) {
			const visible = p < numPlayers;
			for (const mesh of this.pawnMeshes[p]) mesh.visible = visible;
		}
	}

	private snapPawns(state: GameStateView): void {
		// Build occupancy so the ring offset only fires when pawns share a
		// space (start areas initially, home at game-end). A lone pawn on a
		// track / safety tile sits at the tile's exact center.
		const occupancy = new Map<number, Array<[number, number]>>();
		for (let p = 0; p < state.num_players; p++) {
			for (let k = 0; k < PAWNS_PER_PLAYER; k++) {
				const id = state.pawn_positions[p]?.[k];
				if (id === undefined) continue;
				let list = occupancy.get(id);
				if (!list) {
					list = [];
					occupancy.set(id, list);
				}
				list.push([p, k]);
			}
		}

		for (let p = 0; p < state.num_players && p < this.pawnMeshes.length; p++) {
			for (let k = 0; k < PAWNS_PER_PLAYER; k++) {
				const mesh = this.pawnMeshes[p][k];
				const spaceId = state.pawn_positions[p]?.[k];
				if (spaceId === undefined) {
					mesh.visible = false;
					continue;
				}
				const layout = this.geometry.spaces.find((s) => s.id === spaceId);
				if (!layout) continue;
				const [x, z] = toPlanar(layout.center, BOARD_SCALE);
				const sharers = occupancy.get(spaceId) ?? [[p, k]];
				if (sharers.length > 1) {
					const idx = sharers.findIndex(([pp, kk]) => pp === p && kk === k);
					const angle = (idx / sharers.length) * Math.PI * 2;
					mesh.position.set(
						x + Math.cos(angle) * PAWN_STACK_RADIUS,
						PAWN_Y,
						z + Math.sin(angle) * PAWN_STACK_RADIUS
					);
				} else {
					mesh.position.set(x, PAWN_Y, z);
				}
				mesh.visible = true;
			}
		}
	}

	private retintPawns(): void {
		for (let p = 0; p < this.pawnMeshes.length; p++) {
			const color = this.skin.palette.players[p] ?? '#888888';
			for (const mesh of this.pawnMeshes[p]) {
				const mat = mesh.material as THREE.MeshStandardMaterial;
				mat.color.set(color);
			}
		}
	}

	private disposeGroup(group: THREE.Group): void {
		group.traverse((obj) => {
			if ((obj as THREE.Mesh).isMesh) {
				const mesh = obj as THREE.Mesh;
				// Shared pawn geometry must NOT be disposed here — it's cached.
				if (group !== this.piecesGroup) mesh.geometry?.dispose();
				const m = mesh.material;
				if (Array.isArray(m)) m.forEach((mat) => mat.dispose());
				else m?.dispose();
			}
		});
		group.clear();
	}

	private onResize(): void {
		const canvas = this.renderer.domElement;
		const { width, height } = canvas.getBoundingClientRect();
		if (width === 0 || height === 0) return;
		this.renderer.setSize(width, height, false);
		this.camera.aspect = width / height;
		this.camera.updateProjectionMatrix();
	}

	private startLoop(): void {
		if (this.rafId !== null) return;
		const tick = () => {
			this.controls.update();
			this.renderer.render(this.scene, this.camera);
			this.rafId = requestAnimationFrame(tick);
		};
		this.rafId = requestAnimationFrame(tick);
	}

	private stopLoop(): void {
		if (this.rafId !== null) {
			cancelAnimationFrame(this.rafId);
			this.rafId = null;
		}
	}
}
