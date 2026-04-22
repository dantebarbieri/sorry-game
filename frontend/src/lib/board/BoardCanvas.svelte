<script lang="ts">
	import { onMount } from 'svelte';

	import { BoardRenderer, type CameraView } from './renderer';
	import type { BoardGeometry } from './geometry';
	import type { BoardSkin } from './skins';
	import type { GameStateView } from './state';
	import type { PlayRecord } from './actions';

	export interface CameraCommand {
		view: CameraView;
		/** Nonce that increments per click so the same view can re-trigger. */
		nonce: number;
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
		onUserOrbit?: () => void;
		/** Fires when a queued step's animation actually begins. */
		onStepStart?: (step: StepCommand) => void;
		/** Fires when a queued step's animation completes. */
		onStepEnd?: (step: StepCommand) => void;
	}
	let {
		geometry,
		skin,
		gameState,
		lastStep,
		cameraCommand,
		onUserOrbit,
		onStepStart,
		onStepEnd
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
		if (renderer && cameraCommand) renderer.setCameraView(cameraCommand.view);
	});

	$effect(() => {
		if (renderer) renderer.setUserOrbitCallback(onUserOrbit ?? null);
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
