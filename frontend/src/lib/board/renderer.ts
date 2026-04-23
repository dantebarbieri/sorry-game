import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';

import type { BoardGeometry, SpaceId } from './geometry';
import { toPlanar, isTrack } from './geometry';
import type { BoardSkin } from './skins';
import type { GameStateView } from './state';
import type { PlayRecord } from './actions';
import { Animator } from './animator';
import { Interaction, type PickHit } from './interaction';
import { easeInOutCubic, tween } from './timeline';
import {
	activeDestinationDisk,
	boardBase,
	deckStack,
	destinationRing,
	discardStack,
	homePocket,
	lockedDestinationRing,
	safetyChannelSegment,
	selectionHalo,
	slideRamp,
	spacePicker,
	startDisk,
	trackTile
} from './prefabs';
import { cardLabel } from './cards';
import { loadPieceGeometry, makePiece } from './assets';

const BOARD_SCALE = 2.0; // maps [-1, 1] to world [-2, 2]
const PAWNS_PER_PLAYER = 4;
const PAWN_STACK_RADIUS = 0.09;
const PAWN_Y = 0.02;
// Larger picker radii for stacking spaces so clicking anywhere on the
// visible disk registers — not just the small central picker. Matches
// `startDisk` / `homePocket` radii in prefabs.ts.
const PICKER_RADIUS_START = 0.17;
const PICKER_RADIUS_HOME = 0.16;

export type CameraView = 'edge' | 'corner' | 'top';

// Initial camera position — an edge view at a comfortable tilt. The smart-
// snap logic in `setCameraView` preserves the user's current radius and
// polar when the preset change only needs azimuth adjustment.
const INITIAL_CAMERA: [number, number, number] = [0, 4.5, 3.0];
const DEFAULT_TILT = 0.19 * Math.PI; // polar when coming from top-down
const MIN_VISIBLE_TILT = 0.05 * Math.PI; // below this we treat the view as top-down
const TOP_DOWN_POLAR = 0.001; // tiny offset so the up-vector isn't anti-parallel
const CAMERA_TWEEN_MS = 500;

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
	private readonly pickerGroup = new THREE.Group();
	private readonly highlightsGroup = new THREE.Group();
	private readonly deckDiscardGroup = new THREE.Group();
	// Visible Start disks / Home pockets, tagged with `{ spaceId }` so the
	// pointer raycaster treats the whole platform as a pickable space.
	private readonly platformMeshes = new Map<SpaceId, THREE.Mesh>();
	private readonly pickerDisks = new Map<SpaceId, THREE.Mesh>();
	private interaction: Interaction | null = null;
	private pickHandler: ((hit: PickHit | null) => void) | null = null;
	private hoverHandler: ((hit: PickHit | null) => void) | null = null;
	// Monotonic generation counter that invalidates an in-flight camera
	// tween. Bumped on every new `setCameraView` and on user orbit input
	// (`OrbitControls` `start`), so the old tween's per-frame onTick
	// becomes a no-op the moment the user takes over or a new preset
	// is requested.
	private cameraTweenGen = 0;
	private reducedMotion = false;
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
	// The player whose card currently sits on top of the discard. Persisted
	// across `setState` calls so a skin change or no-step refresh still
	// colors the face correctly; reset to null on initial load / New Game.
	private lastDiscardActor: number | null = null;
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
			// Any user grab on OrbitControls cancels an in-flight camera
			// tween — the user has taken over.
			this.cameraTweenGen++;
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
		this.scene.add(this.pickerGroup);
		this.scene.add(this.highlightsGroup);
		this.scene.add(this.deckDiscardGroup);
		this.buildStatic();
		this.buildPickers();

		this.interaction = new Interaction(
			canvas,
			this.camera,
			() => ({
				pawns: this.collectVisiblePawns(),
				// Ray-cast against the visible Start/Home platforms and the
				// invisible track/safety pickers. Having both lets clicks
				// anywhere on a big stacking disk register as that space.
				spaces: [
					...this.platformMeshes.values(),
					...this.pickerDisks.values()
				]
			}),
			(hit) => this.pickHandler?.(hit)
		);
		this.interaction.setHoverCallback((hit) => this.hoverHandler?.(hit));
		this.interaction.attach();

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
		if (this.previousState) this.rebuildDeckDiscard(this.previousState, this.lastDiscardActor);
	}

	setReducedMotion(v: boolean): void {
		this.reducedMotion = v;
		this.animator.setReducedMotion(v);
	}

	/**
	 * Snap the camera to the preset VIEW TYPE nearest its current position,
	 * preserving radius and (when possible) polar. "Edge" snaps azimuth to
	 * the closest multiple of π/2; "corner" to the closest π/4 + k·π/2;
	 * "top" only changes polar. If `requestedAzimuth` is provided, it
	 * overrides the nearest-snap (used to rotate to a specific seat).
	 */
	setCameraView(
		mode: CameraView,
		requestedAzimuth?: number,
		requestedRadius?: number,
		requestedTarget?: [number, number, number]
	): void {
		const startAzimuth = this.controls.getAzimuthalAngle();
		const startPolar = this.controls.getPolarAngle();
		const startRadius = this.camera.position.distanceTo(this.controls.target);
		const endRadius = requestedRadius ?? startRadius;
		// When no radius change is requested, keep the existing radius
		// through the whole tween — preserves the behaviour callers have
		// relied on. Otherwise interpolate to the requested radius.
		const radius = endRadius;

		const startTarget: [number, number, number] = [
			this.controls.target.x,
			this.controls.target.y,
			this.controls.target.z
		];
		const endTarget: [number, number, number] = requestedTarget ?? [0, 0, 0];
		const targetDelta: [number, number, number] = [
			endTarget[0] - startTarget[0],
			endTarget[1] - startTarget[1],
			endTarget[2] - startTarget[2]
		];

		let endAzimuth = startAzimuth;
		let endPolar = startPolar;

		switch (mode) {
			case 'edge':
				endAzimuth = requestedAzimuth ?? snapToNearest(startAzimuth, Math.PI / 2, 0);
				if (startPolar < MIN_VISIBLE_TILT) endPolar = DEFAULT_TILT;
				break;
			case 'corner':
				endAzimuth =
					requestedAzimuth ??
					snapToNearest(startAzimuth, Math.PI / 2, Math.PI / 4);
				if (startPolar < MIN_VISIBLE_TILT) endPolar = DEFAULT_TILT;
				break;
			case 'top':
				endPolar = TOP_DOWN_POLAR;
				break;
		}

		// Take the shortest angular path around the circle so a π↔−π swap
		// goes the near way rather than sweeping 360°.
		let delta = endAzimuth - startAzimuth;
		while (delta > Math.PI) delta -= 2 * Math.PI;
		while (delta < -Math.PI) delta += 2 * Math.PI;
		const wrappedEndAzimuth = startAzimuth + delta;

		const radiusDelta = endRadius - startRadius;
		const targetStill =
			Math.abs(targetDelta[0]) < 1e-4 &&
			Math.abs(targetDelta[1]) < 1e-4 &&
			Math.abs(targetDelta[2]) < 1e-4;
		if (
			this.reducedMotion ||
			(Math.abs(delta) < 1e-4 &&
				Math.abs(endPolar - startPolar) < 1e-4 &&
				Math.abs(radiusDelta) < 1e-4 &&
				targetStill)
		) {
			this.controls.target.set(endTarget[0], endTarget[1], endTarget[2]);
			this.applyCameraSpherical(wrappedEndAzimuth, endPolar, radius);
			return;
		}

		this.cameraTweenGen++;
		const myGen = this.cameraTweenGen;
		void tween(
			CAMERA_TWEEN_MS,
			(t) => {
				if (this.cameraTweenGen !== myGen) return;
				const a = startAzimuth + (wrappedEndAzimuth - startAzimuth) * t;
				const p = startPolar + (endPolar - startPolar) * t;
				const r = startRadius + radiusDelta * t;
				this.controls.target.set(
					startTarget[0] + targetDelta[0] * t,
					startTarget[1] + targetDelta[1] * t,
					startTarget[2] + targetDelta[2] * t
				);
				this.applyCameraSpherical(a, p, r);
			},
			easeInOutCubic
		);
	}

	private applyCameraSpherical(azimuth: number, polar: number, radius: number): void {
		const target = this.controls.target;
		const sinP = Math.sin(polar);
		const cosP = Math.cos(polar);
		this.camera.position.set(
			target.x + radius * sinP * Math.sin(azimuth),
			target.y + radius * cosP,
			target.z + radius * sinP * Math.cos(azimuth)
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
		this.ensurePawns(state);
		// A step implies a fresh actor for the discard top card; a stateless
		// refresh (New Game, reset) clears it so the face reverts to neutral.
		if (player != null) {
			this.lastDiscardActor = player;
		} else if (!record) {
			this.lastDiscardActor = null;
		}
		this.rebuildDeckDiscard(state, this.lastDiscardActor);
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
		this.interaction?.detach();
		this.interaction = null;
		this.controls.dispose();
		this.disposeGroup(this.boardStaticGroup);
		this.disposeGroup(this.piecesGroup);
		this.disposeGroup(this.highlightsGroup);
		this.disposeGroup(this.pickerGroup);
		this.disposeGroup(this.deckDiscardGroup);
		this.pickerDisks.clear();
		this.platformMeshes.clear();
		this.pawnMeshes = [];
		this.renderer.dispose();
	}

	setPickHandler(handler: ((hit: PickHit | null) => void) | null): void {
		this.pickHandler = handler;
	}

	setHoverHandler(handler: ((hit: PickHit | null) => void) | null): void {
		this.hoverHandler = handler;
	}

	/**
	 * Turn pointer-driven picking on or off. Replay mode passes `false`
	 * so raycasts aren't wasted on pointer moves and no pick callback
	 * can ever fire — a hard guarantee that a replay consumer can't
	 * accidentally receive interactive events.
	 */
	setInteractionEnabled(enabled: boolean): void {
		if (!this.interaction) return;
		if (enabled) this.interaction.attach();
		else this.interaction.detach();
	}

	/**
	 * Replace the highlight overlay with rings over `destinations`, an
	 * optional halo under `selectedPawn` (the pawn you're currently
	 * picking a target for), and an optional halo + ring for a locked-in
	 * Split-7 first leg. Pass `{ destinations: [] }` to clear.
	 */
	setHighlights(opts: {
		destinations: SpaceId[];
		activeDestination?: SpaceId | null;
		selectedPawn?: { player: number; pawn: number } | null;
		lockedPawn?: { player: number; pawn: number } | null;
		lockedDestination?: SpaceId | null;
		currentPlayer?: number;
	}): void {
		this.disposeGroup(this.highlightsGroup);
		const state = this.previousState;
		const colorForPlayer = (p: number) => {
			const side = state ? this.sideFor(state, p) : p;
			return new THREE.Color(this.skin.palette.players[side] ?? '#888888');
		};
		const destColor = colorForPlayer(
			opts.currentPlayer ?? opts.selectedPawn?.player ?? opts.lockedPawn?.player ?? 0
		);
		const addDecalAt = (id: SpaceId, mesh: THREE.Mesh) => {
			const layout = this.geometry.spaces.find((s) => s.id === id);
			if (!layout) return;
			const [x, z] = toPlanar(layout.center, BOARD_SCALE);
			mesh.position.x = x;
			mesh.position.z = z;
			this.highlightsGroup.add(mesh);
		};
		// Locked destination (Split-7 first leg) uses a dimmer distinct ring.
		if (opts.lockedDestination != null) {
			addDecalAt(opts.lockedDestination, lockedDestinationRing(destColor));
		}
		// Regular legal destinations — skip the active one; rendered separately.
		for (const id of opts.destinations) {
			if (id === opts.activeDestination) continue;
			if (id === opts.lockedDestination) continue;
			addDecalAt(id, destinationRing(destColor));
		}
		// Active (keyboard-focused) destination — brighter, filled disk.
		if (opts.activeDestination != null) {
			addDecalAt(opts.activeDestination, activeDestinationDisk(destColor));
		}
		const addHalo = (pawn: { player: number; pawn: number }) => {
			const mesh = this.pawnMeshes[pawn.player]?.[pawn.pawn];
			if (!mesh) return;
			const halo = selectionHalo(colorForPlayer(pawn.player));
			halo.position.set(mesh.position.x, halo.position.y, mesh.position.z);
			this.highlightsGroup.add(halo);
		};
		if (opts.lockedPawn) addHalo(opts.lockedPawn);
		if (opts.selectedPawn) addHalo(opts.selectedPawn);
	}

	// ─── Internal ──────────────────────────────────────────────────

	private buildStatic(): void {
		this.platformMeshes.clear();
		this.boardStaticGroup.add(boardBase(this.skin));

		for (const layout of this.geometry.spaces) {
			const mesh = this.meshForSpace(layout.kind);
			if (!mesh) continue;
			const [x, z] = toPlanar(layout.center, BOARD_SCALE);
			mesh.position.x = x;
			mesh.position.z = z;
			mesh.rotation.y = -(layout.tangent_deg * Math.PI) / 180;
			// Tag the visible tile/disk so the pointer raycaster can
			// resolve a click to its SpaceId directly — especially useful
			// for the larger Start and Home platforms where the click
			// area extends well past the invisible picker disk.
			mesh.userData = { spaceId: layout.id };
			this.platformMeshes.set(layout.id, mesh);
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
	}

	/**
	 * Rebuild the deck + discard meshes at the center of the board. The
	 * deck's height scales with `deck_remaining`; the discard's height
	 * scales with `discard.length`, and its top face shows the most
	 * recently-played card colored by the actor who just played it.
	 */
	private rebuildDeckDiscard(state: GameStateView, actor: number | null): void {
		this.disposeGroup(this.deckDiscardGroup);
		if (state.deck_remaining > 0) {
			const deck = deckStack(this.skin, state.deck_remaining);
			deck.position.x = -0.18;
			deck.position.z = 0;
			this.deckDiscardGroup.add(deck);
		}
		if (state.discard.length > 0) {
			const topRaw = state.discard[state.discard.length - 1] ?? null;
			const label = cardLabel(topRaw);
			const actorColor =
				actor != null ? this.skin.palette.players[this.sideFor(state, actor)] : undefined;
			const discard = discardStack(
				this.skin,
				state.discard.length,
				label ?? null,
				actorColor
			);
			discard.position.x = 0.18;
			discard.position.z = 0;
			this.deckDiscardGroup.add(discard);
		}
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

	private buildPickers(): void {
		this.pickerDisks.clear();
		for (const layout of this.geometry.spaces) {
			// StartArea and Home are stacking spaces whose visible disks
			// are larger than a regular track tile — give them a bigger
			// invisible picker so any click on the disk counts.
			let radius: number | undefined;
			if ('StartArea' in layout.kind) radius = PICKER_RADIUS_START;
			else if ('Home' in layout.kind) radius = PICKER_RADIUS_HOME;
			const picker = spacePicker(radius);
			picker.userData = { spaceId: layout.id };
			const [x, z] = toPlanar(layout.center, BOARD_SCALE);
			picker.position.x = x;
			picker.position.z = z;
			this.pickerGroup.add(picker);
			this.pickerDisks.set(layout.id, picker);
		}
	}

	private collectVisiblePawns(): THREE.Object3D[] {
		const out: THREE.Object3D[] = [];
		for (const row of this.pawnMeshes) {
			for (const mesh of row) {
				if (mesh.visible) out.push(mesh);
			}
		}
		return out;
	}

	private sideFor(state: GameStateView, player: number): number {
		return state.seat_sides?.[player] ?? player;
	}

	private ensurePawns(state: GameStateView): void {
		if (!this.pawnGeometry) return;
		while (this.pawnMeshes.length < state.num_players) {
			const p = this.pawnMeshes.length;
			const side = this.sideFor(state, p);
			const color = this.skin.palette.players[side] ?? '#888888';
			const row: THREE.Mesh[] = [];
			for (let k = 0; k < PAWNS_PER_PLAYER; k++) {
				const mesh = makePiece(this.pawnGeometry, color);
				mesh.userData = { player: p, pawn: k };
				this.piecesGroup.add(mesh);
				row.push(mesh);
			}
			this.pawnMeshes.push(row);
		}
		// Hide unused player slots (e.g. 2- or 3-player games on a 4-color board).
		for (let p = 0; p < this.pawnMeshes.length; p++) {
			const visible = p < state.num_players;
			for (const mesh of this.pawnMeshes[p]) mesh.visible = visible;
		}
		// Retint in case seat mapping changed between games (e.g. a new
		// game was started with a different color selection).
		this.retintPawns();
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
		const state = this.previousState;
		for (let p = 0; p < this.pawnMeshes.length; p++) {
			const side = state ? this.sideFor(state, p) : p;
			const color = this.skin.palette.players[side] ?? '#888888';
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
