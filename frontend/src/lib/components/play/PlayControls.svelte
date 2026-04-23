<script lang="ts">
	import type { CameraView } from '$lib/board/renderer';

	interface Props {
		canPass: boolean;
		canCancelSplit: boolean;
		stepping: boolean;
		gameOver: boolean;
		canStepBot: boolean;
		autoStep: boolean;
		autoPass: boolean;
		preferSwap11: boolean;
		activePreset: CameraView | null;
		onPass: () => void;
		onCancelSplit: () => void;
		onStepBot: () => void;
		onToggleAutoStep: () => void;
		onToggleAutoPass: () => void;
		onTogglePreferSwap11: () => void;
		onNewGame: () => void;
		onPickView: (view: CameraView) => void;
	}

	const {
		canPass,
		canCancelSplit,
		stepping,
		gameOver,
		canStepBot,
		autoStep,
		autoPass,
		preferSwap11,
		activePreset,
		onPass,
		onCancelSplit,
		onStepBot,
		onToggleAutoStep,
		onToggleAutoPass,
		onTogglePreferSwap11,
		onNewGame,
		onPickView
	}: Props = $props();
</script>

<div class="controls" role="toolbar" aria-label="Game controls">
	<div class="group" aria-label="Camera">
		<button onclick={() => onPickView('edge')} class:active={activePreset === 'edge'}>Edge</button>
		<button onclick={() => onPickView('corner')} class:active={activePreset === 'corner'}>Corner</button>
		<button onclick={() => onPickView('top')} class:active={activePreset === 'top'}>Top-down</button>
	</div>
	<div class="group" aria-label="Game">
		{#if canCancelSplit}
			<button onclick={onCancelSplit} disabled={stepping}>Cancel split</button>
		{/if}
		{#if canPass}
			<button onclick={onPass} disabled={stepping}>Pass</button>
		{/if}
		<button onclick={onStepBot} disabled={!canStepBot}>Step bot</button>
		<button
			onclick={onToggleAutoStep}
			class:active={autoStep}
			disabled={gameOver}
			aria-pressed={autoStep}
			title="Let bots play their turns automatically"
		>
			{autoStep ? 'Auto-step: on' : 'Auto-step: off'}
		</button>
		<button
			onclick={onToggleAutoPass}
			class:active={autoPass}
			disabled={gameOver}
			aria-pressed={autoPass}
			title="Auto-pass when Pass is your only legal option"
		>
			{autoPass ? 'Auto-pass: on' : 'Auto-pass: off'}
		</button>
		<button
			onclick={onTogglePreferSwap11}
			class:active={preferSwap11}
			disabled={gameOver}
			aria-pressed={preferSwap11}
			title="On an 11, clicking an opponent exactly 11 spaces away resolves to Swap (on) or Bump (off)"
		>
			11 @ 11: {preferSwap11 ? 'Swap' : 'Bump'}
		</button>
		<button onclick={onNewGame} class="new-game">New game</button>
	</div>
</div>

<style>
	.controls {
		display: flex;
		gap: 0.75rem;
		padding: 0.45rem 1rem;
		flex-wrap: wrap;
		background: rgba(0, 0, 0, 0.18);
		border-bottom: 1px solid rgba(255, 255, 255, 0.06);
		flex: 0 0 auto;
	}
	:global(.app[data-skin='light']) .controls {
		background: rgba(0, 0, 0, 0.04);
		border-bottom-color: rgba(0, 0, 0, 0.08);
	}
	.group {
		display: flex;
		gap: 0.3rem;
		padding: 0.15rem;
		border-radius: 6px;
		background: rgba(0, 0, 0, 0.25);
	}
	:global(.app[data-skin='light']) .group {
		background: rgba(255, 255, 255, 0.4);
	}
	button {
		background: transparent;
		color: inherit;
		border: 0;
		padding: 0.3rem 0.75rem;
		cursor: pointer;
		border-radius: 4px;
		font: inherit;
		font-size: 0.85rem;
	}
	button:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.08);
	}
	:global(.app[data-skin='light']) button:hover:not(:disabled) {
		background: rgba(0, 0, 0, 0.06);
	}
	button.active {
		background: rgba(246, 196, 84, 0.22);
	}
	button:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}
	.new-game {
		background: rgba(246, 196, 84, 0.2);
		font-weight: 600;
	}
</style>
