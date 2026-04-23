<script lang="ts">
	import { cardLabel } from '$lib/board/cards';
	import type { BoardSkin } from '$lib/board/skins';
	import type { GameStateView } from '$lib/board/state';
	import type { StepCommand } from '$lib/board/BoardCanvas.svelte';
	import { PLAYER_NAMES, PLACEMENT_ORDINAL, type ViewerSeat } from '$lib/play/types';
	import type { SplitLeg } from '$lib/board/actions';

	interface Props {
		skin: BoardSkin;
		gameState: GameStateView | null;
		lastStep: StepCommand | undefined;
		viewer: ViewerSeat;
		stepping: boolean;
		viewerCanMove: boolean;
		pendingLeg1: SplitLeg | null;
		lastAnnouncement: string;
	}

	const {
		skin,
		gameState,
		lastStep,
		viewer,
		viewerCanMove,
		pendingLeg1,
		lastAnnouncement
	}: Props = $props();

	const turnLabel = $derived.by(() => {
		const s = gameState;
		if (!s) return '';
		if (s.winners.length > 0)
			return `Game over — ${PLAYER_NAMES[s.winners[0]] ?? `P${s.winners[0]}`} wins`;
		if (s.truncated) return 'Game over (truncated)';
		return `Turn ${s.turn_count + 1} · ${PLAYER_NAMES[s.current_player] ?? `P${s.current_player}`}`;
	});

	const lastPlayedLabel = $derived.by(() => {
		const step = lastStep;
		if (!step) return null;
		return {
			label: cardLabel(step.record.card) ?? step.record.card,
			color: skin.palette.players[step.player]
		};
	});

	const pawnsHome = $derived.by(() => {
		const s = gameState;
		if (!s) return [] as number[];
		// A pawn is home when its space is this player's Home space. We
		// don't have direct access to the geometry here, so rely on the
		// convention that Home is the final space reachable from safety —
		// defer to a simple fallback: count pawns at the same "home" space
		// tag is awkward without geometry, so expose just pawn positions'
		// distinct-from-start count as a proxy until a better signal exists.
		// For now: the engine already lists winners when 4 pawns are home,
		// and partial progress isn't numerically shown.
		const res: number[] = new Array(s.num_players).fill(0);
		// Count pawns that are *not* on a Start-like space using a tag
		// lookup would require geometry; instead, show an approximation
		// from `winners`: a winning player has all 4 home.
		for (const w of s.winners) res[w] = 4;
		return res;
	});

	const drawnCard = $derived.by(() => {
		const s = gameState;
		if (!s || s.winners.length > 0 || s.truncated) return null;
		if (viewer !== null && s.current_player !== viewer) return null;
		return s.drawn_card ? cardLabel(s.drawn_card) ?? s.drawn_card : null;
	});
</script>

<div class="hud">
	<div class="lhs">
		<span class="turn" aria-live="polite">{turnLabel}</span>
		{#if lastPlayedLabel}
			<span class="card-chip" style:color={lastPlayedLabel.color}>
				{lastPlayedLabel.label}
			</span>
		{/if}
		{#if viewerCanMove && viewer !== null}
			<span class="you-up" style:color={skin.palette.players[viewer]}>
				Your turn ({drawnCard ?? '—'})
			</span>
		{/if}
		{#if pendingLeg1}
			<span class="split-status">
				Split 7: leg 1 = pawn {pendingLeg1.pawn} → {pendingLeg1.to} ({pendingLeg1.steps} steps). Pick a different pawn + destination for leg 2.
			</span>
		{/if}
	</div>

	<div class="rhs">
		{#if gameState && !gameState.winners.length && !gameState.truncated}
			<div class="seat-strip" aria-label="Seats">
				{#each Array(gameState.num_players) as _, p (p)}
					<span
						class="seat-dot"
						class:current={p === gameState.current_player}
						style:background={skin.palette.players[p]}
						title="{PLAYER_NAMES[p] ?? `P${p}`}{p === gameState.current_player ? ' (up)' : ''}"
					></span>
				{/each}
			</div>
		{/if}
		{#if gameState && gameState.winners.length > 0}
			<div class="placements" role="status">
				{#each gameState.winners as player, i (player)}
					<span class="place" style:color={skin.palette.players[player]}>
						{PLACEMENT_ORDINAL[i] ?? `${i + 1}th`} {PLAYER_NAMES[player] ?? `P${player}`}
					</span>
				{/each}
			</div>
		{/if}
	</div>

	<div class="visually-hidden" aria-live="polite" aria-atomic="true">
		{lastAnnouncement}
	</div>
</div>

<style>
	.hud {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.5rem 1rem;
		flex-wrap: wrap;
		background: rgba(0, 0, 0, 0.25);
		border-bottom: 1px solid rgba(255, 255, 255, 0.08);
		flex: 0 0 auto;
	}
	:global(.app[data-skin='light']) .hud {
		background: rgba(0, 0, 0, 0.05);
		border-bottom-color: rgba(0, 0, 0, 0.08);
	}
	.lhs {
		display: flex;
		gap: 0.75rem;
		align-items: baseline;
		flex-wrap: wrap;
		flex: 1;
		min-width: 0;
	}
	.rhs {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}
	.turn {
		font-size: 0.95rem;
		font-weight: 600;
	}
	.card-chip {
		font-weight: 700;
		font-size: 1.4rem;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0.02em;
		padding: 0.1rem 0.6rem;
		border-radius: 4px;
		background: rgba(0, 0, 0, 0.3);
	}
	:global(.app[data-skin='light']) .card-chip {
		background: rgba(255, 255, 255, 0.6);
	}
	.you-up {
		font-weight: 600;
		font-size: 0.9rem;
	}
	.split-status {
		font-size: 0.85rem;
		color: #f2db88;
		font-family: ui-monospace, SFMono-Regular, monospace;
	}
	.seat-strip {
		display: flex;
		gap: 0.3rem;
	}
	.seat-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		opacity: 0.55;
	}
	.seat-dot.current {
		opacity: 1;
		outline: 2px solid rgba(246, 196, 84, 0.7);
		outline-offset: 1px;
	}
	.placements {
		display: flex;
		gap: 0.5rem;
		font-size: 0.85rem;
		font-weight: 600;
	}
	.place {
		padding: 0.15rem 0.4rem;
		border-radius: 3px;
		background: rgba(0, 0, 0, 0.35);
	}
	.visually-hidden {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
