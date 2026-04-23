<script lang="ts">
	import { onMount } from 'svelte';
	import BoardCanvas, { type StepCommand } from '$lib/board/BoardCanvas.svelte';
	import type { BoardGeometry } from '$lib/board/geometry';
	import type { GameHistory } from '$lib/board/actions';
	import { HistoryCursor, type PlayBeat } from '$lib/board/history-cursor';
	import type { ReplayState } from '$lib/board/replay';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';
	import { PLAYER_NAMES } from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';

	interface Props {
		history: GameHistory;
		onClose: () => void;
	}

	const { history, onClose }: Props = $props();

	const theme = useTheme();

	let geometry = $state<BoardGeometry | null>(null);
	let cursor = $state<HistoryCursor | null>(null);
	let viewState = $state<ReplayState | null>(null);
	let lastStep = $state<StepCommand | undefined>(undefined);
	let index = $state(0);
	let length = $state(0);
	let animating = $state(false);
	let autoPlay = $state(false);
	let error = $state<string | null>(null);
	let nonce = 0;

	onMount(async () => {
		try {
			const wasm = await loadWasm();
			geometry = parseJsonOrThrow<BoardGeometry>(wasm.get_board_geometry('Standard'));
			const c = new HistoryCursor(history, geometry);
			cursor = c;
			viewState = c.currentState;
			index = c.index;
			length = c.length;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	});

	function stepForward() {
		if (!cursor || cursor.isAtEnd || animating) return;
		const beat = cursor.stepForward();
		if (!beat) return;
		applyBeat(beat);
	}

	function applyBeat(beat: PlayBeat) {
		animating = true;
		viewState = beat.next;
		lastStep = { record: beat.record, player: beat.player, nonce: ++nonce };
		index = cursor?.index ?? 0;
	}

	function onStepEnd() {
		animating = false;
		if (autoPlay && cursor && !cursor.isAtEnd) stepForward();
	}

	function stepBack() {
		if (!cursor || cursor.isAtStart || animating) return;
		const state = cursor.stepBack();
		viewState = state;
		lastStep = undefined;
		index = cursor.index;
	}

	function reset() {
		if (!cursor || animating) return;
		const state = cursor.jumpTo(0);
		viewState = state;
		lastStep = undefined;
		index = 0;
		autoPlay = false;
	}

	function toEnd() {
		if (!cursor || animating) return;
		const state = cursor.jumpTo(cursor.length);
		viewState = state;
		lastStep = undefined;
		index = cursor.length;
	}

	function toggleAutoplay() {
		autoPlay = !autoPlay;
		if (autoPlay && cursor && !cursor.isAtEnd && !animating) stepForward();
	}

	function onScrub(e: Event) {
		if (!cursor || animating) return;
		const target = e.currentTarget as HTMLInputElement;
		const t = Number(target.value);
		const state = cursor.jumpTo(t);
		viewState = state;
		lastStep = undefined;
		index = cursor.index;
	}

	const winnerLabel = $derived.by(() => {
		if (history.winners.length === 0) return 'No winner recorded';
		return `Winner: ${PLAYER_NAMES[history.winners[0]] ?? `P${history.winners[0]}`}`;
	});
</script>

<div class="scrim" role="presentation" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()}></div>
<div class="drawer" role="dialog" aria-modal="true" aria-label="Game replay">
	<header>
		<div>
			<h2>Replay</h2>
			<p class="meta">Seed {history.seed} · {winnerLabel}</p>
		</div>
		<button class="close" onclick={onClose} aria-label="Close replay">✕</button>
	</header>

	<div class="board-wrap">
		{#if error}
			<p class="msg error">Error: {error}</p>
		{:else if !geometry || !viewState || !cursor}
			<p class="msg">Loading replay…</p>
		{:else}
			<BoardCanvas
				{geometry}
				skin={theme.skin}
				gameState={viewState}
				{lastStep}
				mode="replay"
				onStepEnd={onStepEnd}
			/>
		{/if}
	</div>

	<div class="controls">
		<button onclick={reset} disabled={!cursor || cursor.isAtStart || animating}>⏮</button>
		<button onclick={stepBack} disabled={!cursor || cursor.isAtStart || animating}>←</button>
		<button onclick={toggleAutoplay} class:active={autoPlay} disabled={!cursor || cursor.isAtEnd}>
			{autoPlay ? '⏸' : '▶'}
		</button>
		<button onclick={stepForward} disabled={!cursor || cursor.isAtEnd || animating}>→</button>
		<button onclick={toEnd} disabled={!cursor || cursor.isAtEnd}>⏭</button>
		<input
			type="range"
			min="0"
			max={length}
			value={index}
			oninput={onScrub}
			aria-label="Timeline"
			disabled={animating}
		/>
		<span class="progress">{index} / {length}</span>
	</div>
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.55);
		z-index: 100;
	}
	.drawer {
		position: fixed;
		top: 3.5rem;
		left: 50%;
		transform: translateX(-50%);
		width: min(60rem, 95vw);
		max-height: calc(100vh - 5rem);
		background: #141c28;
		color: #e8ecf2;
		border: 1px solid rgba(255, 255, 255, 0.08);
		border-radius: 10px;
		padding: 1rem 1.25rem;
		box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
		z-index: 101;
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	:global(.app[data-skin='light']) .drawer {
		background: #faf4e0;
		color: #23201a;
		border-color: rgba(0, 0, 0, 0.12);
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
	}
	h2 {
		margin: 0;
		font-size: 1.1rem;
	}
	.meta {
		margin: 0.15rem 0 0;
		font-size: 0.8rem;
		opacity: 0.65;
	}
	.close {
		background: transparent;
		border: 0;
		color: inherit;
		font-size: 1.2rem;
		cursor: pointer;
		padding: 0.2rem 0.5rem;
	}
	.board-wrap {
		position: relative;
		height: 24rem;
		background: rgba(0, 0, 0, 0.3);
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
	.controls {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.controls button {
		background: transparent;
		color: inherit;
		border: 1px solid currentColor;
		padding: 0.3rem 0.65rem;
		border-radius: 4px;
		font: inherit;
		font-size: 0.9rem;
		cursor: pointer;
		opacity: 0.8;
	}
	.controls button:hover:not(:disabled) {
		opacity: 1;
	}
	.controls button:disabled {
		opacity: 0.3;
		cursor: not-allowed;
	}
	.controls button.active {
		background: rgba(246, 196, 84, 0.25);
		border-color: rgba(246, 196, 84, 0.7);
	}
	.controls input[type='range'] {
		flex: 1;
		min-width: 0;
	}
	.progress {
		font-family: ui-monospace, monospace;
		font-size: 0.8rem;
		opacity: 0.65;
		font-variant-numeric: tabular-nums;
		min-width: 4.5rem;
		text-align: right;
	}
</style>
