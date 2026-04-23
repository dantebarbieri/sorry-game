<script lang="ts">
	import { onMount } from 'svelte';
	import BoardCanvas, {
		type CameraCommand,
		type StepCommand
	} from '$lib/board/BoardCanvas.svelte';
	import type { BoardGeometry } from '$lib/board/geometry';
	import type { GameStateView } from '$lib/board/state';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';
	import type { DemoSpec } from '$lib/rules/demo-specs';
	import { buildDemoStates } from '$lib/rules/demo-specs';
	import { useTheme } from '$lib/theme-context.svelte';

	interface Props {
		spec: DemoSpec;
	}

	const { spec }: Props = $props();

	const theme = useTheme();

	let geometry = $state<BoardGeometry | null>(null);
	let states = $state<GameStateView[]>([]);
	let resolvedBeats = $state<import('$lib/board/actions').PlayRecord[]>([]);
	let step = $state(0);
	let lastStep = $state<StepCommand | undefined>(undefined);
	let cameraCommand = $state<CameraCommand | undefined>(undefined);
	let error = $state<string | null>(null);
	let nonce = 0;

	onMount(async () => {
		try {
			const wasm = await loadWasm();
			geometry = parseJsonOrThrow<BoardGeometry>(wasm.get_board_geometry('Standard'));
			const built = buildDemoStates(spec, geometry);
			states = built.states;
			resolvedBeats = built.resolvedBeats;
			step = 0;
			// Set the camera view from the spec (default = corner).
			cameraCommand = {
				view: spec.cameraView ?? 'corner',
				nonce: ++nonce,
				targetAzimuth: spec.cameraAzimuth
			};
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	});

	function forward() {
		if (step >= spec.beats.length) return;
		const beat = spec.beats[step];
		const record = resolvedBeats[step];
		lastStep = {
			record,
			player: beat.player,
			nonce: ++nonce
		};
		step++;
	}

	function back() {
		if (step === 0) return;
		step--;
		lastStep = undefined;
	}

	function reset() {
		step = 0;
		lastStep = undefined;
	}

	function onKeyDown(e: KeyboardEvent) {
		// Ignore keystrokes that are meant for a focusable form control
		// inside the demo (unlikely here, but future-proof).
		const target = e.target as Element | null;
		if (target && (target.tagName === 'INPUT' || target.tagName === 'SELECT' || target.tagName === 'TEXTAREA')) {
			return;
		}
		if (e.key === 'ArrowRight') {
			e.preventDefault();
			forward();
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			back();
		} else if (e.key === 'Home') {
			e.preventDefault();
			reset();
		}
	}

	const currentState = $derived(states[step] ?? null);
	const currentBeat = $derived(step > 0 ? spec.beats[step - 1] : null);
	const atEnd = $derived(step >= spec.beats.length);
	const atStart = $derived(step === 0);
</script>

<!-- Focusable region so keyboard users can drive the stepper with arrows.
     `role="group"` + aria-label is the ARIA-correct wrapper for a cluster
     of related controls; the a11y-rule ignores are deliberate. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
	class="demo"
	role="group"
	aria-labelledby="demo-{spec.id}-title"
	tabindex="0"
	onkeydown={onKeyDown}
>
	<header>
		<h3 id="demo-{spec.id}-title">{spec.title}</h3>
	</header>
	<p class="body">{spec.body}</p>
	<div class="board-wrap">
		{#if error}
			<p class="msg error">Error: {error}</p>
		{:else if !geometry || !currentState}
			<p class="msg">Loading…</p>
		{:else}
			<BoardCanvas
				{geometry}
				skin={theme.skin}
				gameState={currentState}
				{lastStep}
				{cameraCommand}
				mode="replay"
			/>
		{/if}
	</div>
	<footer class="demo-controls">
		<div class="stepper" aria-live="polite">
			{#if spec.beats.length > 0}
				<span class="counter">{step} / {spec.beats.length}</span>
				<span class="label">
					{#if currentBeat}{currentBeat.label}{:else}Initial position{/if}
				</span>
			{:else}
				<span class="label">No beats — static scene.</span>
			{/if}
		</div>
		{#if spec.beats.length > 0}
			<div class="buttons" role="group" aria-label="Demo stepper">
				<button onclick={back} disabled={atStart} aria-label="Previous beat">← Back</button>
				<button onclick={reset} disabled={atStart}>Reset</button>
				<button onclick={forward} disabled={atEnd} aria-label="Next beat" class="primary">
					{atEnd ? 'Done' : 'Next →'}
				</button>
			</div>
		{/if}
	</footer>
</section>

<style>
	.demo {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1rem;
		border-radius: 10px;
		background: rgba(255, 255, 255, 0.03);
		border: 1px solid rgba(255, 255, 255, 0.08);
	}
	:global(.app[data-skin='light']) .demo {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.08);
	}
	.demo:focus-visible {
		outline: 2px solid rgba(246, 196, 84, 0.7);
		outline-offset: 2px;
	}
	header h3 {
		margin: 0;
		font-size: 1.05rem;
	}
	.body {
		margin: 0;
		opacity: 0.85;
		line-height: 1.5;
	}
	.board-wrap {
		position: relative;
		height: 22rem;
		background: rgba(0, 0, 0, 0.25);
		border-radius: 6px;
		overflow: hidden;
	}
	:global(.app[data-skin='light']) .board-wrap {
		background: rgba(0, 0, 0, 0.05);
	}
	.msg {
		padding: 1.5rem;
		text-align: center;
	}
	.error {
		color: salmon;
	}
	.demo-controls {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		flex-wrap: wrap;
	}
	.stepper {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex: 1;
		min-width: 0;
	}
	.counter {
		font-family: ui-monospace, monospace;
		font-size: 0.85rem;
		opacity: 0.65;
		font-variant-numeric: tabular-nums;
	}
	.label {
		font-size: 0.9rem;
		opacity: 0.85;
	}
	.buttons {
		display: flex;
		gap: 0.4rem;
	}
	button {
		background: transparent;
		color: inherit;
		border: 1px solid currentColor;
		padding: 0.3rem 0.7rem;
		border-radius: 4px;
		font: inherit;
		font-size: 0.85rem;
		cursor: pointer;
		opacity: 0.8;
	}
	button:hover:not(:disabled) {
		opacity: 1;
	}
	button:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}
	button.primary {
		background: rgba(246, 196, 84, 0.25);
		border-color: rgba(246, 196, 84, 0.7);
		font-weight: 600;
	}
</style>
