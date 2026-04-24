<script lang="ts">
	import { cardLabel } from '$lib/board/cards';
	import type { BoardSkin } from '$lib/board/skins';
	import type { GameStateView } from '$lib/board/state';
	import { PLAYER_NAMES, type ViewerSeat } from '$lib/play/types';
	import type { SplitLeg } from '$lib/board/actions';

	interface Props {
		skin: BoardSkin;
		gameState: GameStateView | null;
		viewer: ViewerSeat;
		stepping: boolean;
		pendingLeg1: SplitLeg | null;
	}

	const { skin, gameState, viewer, stepping, pendingLeg1 }: Props = $props();

	function sideOf(p: number): number {
		return gameState?.seat_sides?.[p] ?? p;
	}

	// Only shown while the engine is mid-turn (neither GameOver nor blank
	// pre-game state). Sources the face-up card from `action_needed.card`
	// directly so *every* viewer sees it — not the top-level `drawn_card`
	// which is redundant (and previously gated by viewer).
	const shown = $derived.by(() => {
		const s = gameState;
		if (!s) return null;
		if (s.winners.length > 0 || s.truncated) return null;
		const an = s.action_needed;
		const side = sideOf(s.current_player);
		const playerName = PLAYER_NAMES[side] ?? `P${side}`;
		const playerColor = skin.palette.players[side] ?? '#888';
		const isYou = viewer !== null && s.current_player === viewer;

		if (an.type === 'ChooseMove') {
			const status = pendingLeg1
				? `Split 7 — pick leg 2`
				: stepping
					? 'Bot thinking…'
					: isYou
						? 'Choose a move'
						: `${playerName} is playing`;
			return {
				face: 'face' as const,
				label: cardLabel(an.card) ?? an.card,
				playerName,
				playerColor,
				isYou,
				status
			};
		}
		if (an.type === 'ChooseCard') {
			return {
				face: 'back' as const,
				label: null,
				playerName,
				playerColor,
				isYou,
				status: isYou ? 'Pick a card to play' : `${playerName} is picking a card`
			};
		}
		return null;
	});
</script>

{#if shown}
	<div
		class="active-card"
		style:--accent={shown.playerColor}
		aria-live="polite"
		aria-atomic="true"
	>
		{#key `${gameState?.current_player}-${shown.label ?? 'back'}-${shown.face}`}
			<div class="card-face" class:back={shown.face === 'back'}>
				{#if shown.face === 'face' && shown.label}
					{#if shown.label === 'Sorry!'}
						<span class="label sorry">{shown.label}</span>
					{:else}
						<span class="label" class:small={shown.label.length > 2}>{shown.label}</span>
					{/if}
				{:else}
					<span class="back-mark" aria-hidden="true">✕</span>
				{/if}
			</div>
		{/key}
		<div class="meta">
			<div class="who">
				{#if shown.isYou}
					<strong class="you" style:color={shown.playerColor}>Your turn</strong>
					<span class="dim">· {shown.playerName}</span>
				{:else}
					<strong style:color={shown.playerColor}>{shown.playerName}</strong>
				{/if}
			</div>
			<div class="status">{shown.status}</div>
		</div>
	</div>
{/if}

<style>
	.active-card {
		position: absolute;
		top: 0.65rem;
		left: 0.75rem;
		z-index: 3;
		pointer-events: none;
		display: flex;
		align-items: center;
		gap: 0.65rem;
		padding: 0.45rem 0.7rem 0.45rem 0.5rem;
		border-radius: 10px;
		background: rgba(0, 0, 0, 0.42);
		box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
		backdrop-filter: blur(6px);
		color: #f6f1de;
	}
	:global(.app[data-skin='light']) .active-card {
		background: rgba(255, 255, 255, 0.75);
		color: #1e1b16;
		box-shadow: 0 6px 16px rgba(0, 0, 0, 0.18);
	}

	.card-face {
		flex: 0 0 auto;
		width: 64px;
		height: 90px;
		border-radius: 10px;
		background: #fbf6e7;
		border: 4px solid var(--accent);
		box-shadow: 0 2px 6px rgba(0, 0, 0, 0.28);
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
		position: relative;
		animation: pop-in 260ms ease-out;
	}
	.card-face.back {
		background: repeating-linear-gradient(
			45deg,
			rgba(0, 0, 0, 0.22),
			rgba(0, 0, 0, 0.22) 4px,
			rgba(0, 0, 0, 0.12) 4px,
			rgba(0, 0, 0, 0.12) 8px
		);
		border-color: rgba(0, 0, 0, 0.4);
	}
	.label {
		font-size: 2.5rem;
		font-weight: 800;
		font-variant-numeric: tabular-nums;
		color: var(--accent);
		line-height: 1;
	}
	.label.small {
		font-size: 1.35rem;
		letter-spacing: -0.01em;
	}
	.label.sorry {
		font-size: 1.35rem;
		font-weight: 800;
		letter-spacing: 0.01em;
		transform: rotate(-60deg);
		white-space: nowrap;
	}
	.back-mark {
		font-size: 1.4rem;
		color: rgba(255, 255, 255, 0.55);
		font-weight: 700;
	}

	.meta {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		min-width: 0;
	}
	.who {
		font-size: 1rem;
		font-weight: 700;
		letter-spacing: 0.01em;
	}
	.you {
		text-transform: uppercase;
		font-size: 0.95rem;
		letter-spacing: 0.05em;
	}
	.dim {
		opacity: 0.75;
		font-weight: 500;
		margin-left: 0.15rem;
	}
	.status {
		font-size: 0.8rem;
		opacity: 0.8;
	}

	@keyframes pop-in {
		from {
			transform: scale(0.72) rotate(-6deg);
			opacity: 0;
		}
		60% {
			transform: scale(1.04) rotate(0);
			opacity: 1;
		}
		to {
			transform: scale(1) rotate(0);
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.card-face {
			animation: none;
		}
	}
</style>
