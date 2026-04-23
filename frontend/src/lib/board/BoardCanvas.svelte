<script lang="ts">
	import { onMount } from 'svelte';

	import { BoardRenderer, type CameraView } from './renderer';
	import type { BoardGeometry, SpaceId } from './geometry';
	import type { BoardSkin } from './skins';
	import type { GameStateView } from './state';
	import type { PlayRecord } from './actions';
	import type { PickHit } from './interaction';

	export interface HighlightState {
		destinations: SpaceId[];
		/** Currently-focused destination (keyboard nav / hover). Rendered
		 *  distinctly from the base destinations. */
		activeDestination?: SpaceId | null;
		selectedPawn?: { player: number; pawn: number } | null;
		/** Additional pawn haloed as "locked in" (e.g. a Split-7 first leg's
		 *  pawn while the user is picking the second leg). */
		lockedPawn?: { player: number; pawn: number } | null;
		/** Fixed destination ring styled as "already committed" — used for a
		 *  Split-7 locked first-leg target. */
		lockedDestination?: SpaceId | null;
		currentPlayer?: number;
	}

	export interface CameraCommand {
		view: CameraView;
		/** Nonce that increments per click so the same view can re-trigger. */
		nonce: number;
		/** Override azimuth (radians) for `edge` / `corner` — skips the
		 *  nearest-snap. Used to rotate to a specific seat. */
		targetAzimuth?: number;
	}

	export interface StepCommand {
		record: PlayRecord;
		player: number;
		/** Increments per step so repeat-clicks still trigger animation. */
		nonce: number;
	}

	interface Props {
		geometry: BoardGeometry;
		skin: BoardSkin;
		gameState?: GameStateView;
		lastStep?: StepCommand;
		cameraCommand?: CameraCommand;
		highlights?: HighlightState;
		onUserOrbit?: () => void;
		/** Fires when a queued step's animation actually begins. */
		onStepStart?: (step: StepCommand) => void;
		/** Fires when a queued step's animation completes. */
		onStepEnd?: (step: StepCommand) => void;
		/** Fires on pointer-down; `null` = clicked empty space / board. */
		onPick?: (hit: PickHit | null) => void;
		/** Fires on pointer-move; `null` = hovering over nothing pickable. */
		onHover?: (hit: PickHit | null) => void;
	}
	let {
		geometry,
		skin,
		gameState,
		lastStep,
		cameraCommand,
		highlights,
		onUserOrbit,
		onStepStart,
		onStepEnd,
		onPick,
		onHover
	}: Props = $props();

	let canvas: HTMLCanvasElement | undefined = $state();
	let renderer: BoardRenderer | undefined = $state();
	let reducedMotion = $state(false);

	// The last step-nonce we've forwarded to the animator. Lets the reactive
	// $effect know when the `gameState` change carries a *new* step (→ animate)
	// vs. an initial/reset state (→ snap).
	let processedStepNonce = -1;

	onMount(() => {
		if (!canvas) return;
		const r = new BoardRenderer(canvas, geometry, skin);
		renderer = r;
		const mm = window.matchMedia('(prefers-reduced-motion: reduce)');
		reducedMotion = mm.matches;
		r.setReducedMotion(reducedMotion);
		const onMediaChange = () => {
			reducedMotion = mm.matches;
			r.setReducedMotion(reducedMotion);
		};
		mm.addEventListener('change', onMediaChange);
		return () => {
			mm.removeEventListener('change', onMediaChange);
			r.dispose();
			renderer = undefined;
		};
	});

	$effect(() => {
		if (renderer) renderer.setSkin(skin);
	});

	$effect(() => {
		if (!renderer || !gameState) return;
		const step = lastStep;
		if (step && step.nonce !== processedStepNonce) {
			processedStepNonce = step.nonce;
			// Capture the step so the callbacks carry *this* step's info
			// rather than whatever `lastStep` has drifted to by the time
			// the animator dequeues it.
			const captured = step;
			renderer.setState(gameState, step.record, step.player, {
				onStart: () => onStepStart?.(captured),
				onDone: () => onStepEnd?.(captured)
			});
		} else {
			renderer.setState(gameState);
		}
	});

	$effect(() => {
		if (renderer && cameraCommand) {
			renderer.setCameraView(cameraCommand.view, cameraCommand.targetAzimuth);
		}
	});

	$effect(() => {
		if (renderer) renderer.setUserOrbitCallback(onUserOrbit ?? null);
	});

	$effect(() => {
		if (renderer) renderer.setPickHandler(onPick ?? null);
	});

	$effect(() => {
		if (renderer) renderer.setHoverHandler(onHover ?? null);
	});

	$effect(() => {
		if (!renderer) return;
		const h = highlights;
		if (!h) {
			renderer.setHighlights({ destinations: [] });
		} else {
			renderer.setHighlights({
				destinations: h.destinations,
				activeDestination: h.activeDestination ?? null,
				selectedPawn: h.selectedPawn ?? null,
				lockedPawn: h.lockedPawn ?? null,
				lockedDestination: h.lockedDestination ?? null,
				currentPlayer: h.currentPlayer
			});
		}
	});
</script>

<canvas bind:this={canvas} class="board-canvas"></canvas>

<style>
	.board-canvas {
		display: block;
		width: 100%;
		height: 100%;
	}
</style>
