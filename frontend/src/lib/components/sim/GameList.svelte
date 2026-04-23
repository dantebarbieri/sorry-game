<script lang="ts">
	import type { GameSummary } from '$lib/simulate/types';
	import { PLAYER_NAMES } from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';

	interface Props {
		games: GameSummary[];
		selected: number | null;
		onSelect: (index: number) => void;
		canReplay: boolean;
	}

	const { games, selected, onSelect, canReplay }: Props = $props();
	const theme = useTheme();
</script>

<div class="wrap">
	<header>
		<h3>Games</h3>
		<span class="count">{games.length}</span>
	</header>
	<ul>
		{#each games as g (g.index)}
			<li>
				<button
					type="button"
					onclick={() => onSelect(g.index)}
					class:selected={selected === g.index}
					disabled={!canReplay}
					title={canReplay ? 'Replay this game' : 'Enable "keep histories" to replay'}
				>
					<span class="idx">#{g.index + 1}</span>
					<span class="winner">
						{#if g.winners.length > 0}
							<span
								class="dot"
								style:background={theme.skin.palette.players[g.winners[0]]}
								aria-hidden="true"
							></span>
							{PLAYER_NAMES[g.winners[0]] ?? `P${g.winners[0]}`}
						{:else}
							—
						{/if}
					</span>
					<span class="turns">{g.num_turns}t</span>
					{#if g.truncated}
						<span class="flag">truncated</span>
					{/if}
				</button>
			</li>
		{/each}
	</ul>
	{#if games.length === 0}
		<p class="empty">Completed games will appear here.</p>
	{/if}
</div>

<style>
	.wrap {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		max-height: 30rem;
		overflow-y: auto;
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		padding: 0 0.3rem;
	}
	h3 {
		margin: 0;
		font-size: 0.9rem;
	}
	.count {
		font-size: 0.78rem;
		opacity: 0.6;
		font-variant-numeric: tabular-nums;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}
	button {
		display: grid;
		grid-template-columns: 2.5rem 1fr auto auto;
		align-items: center;
		gap: 0.6rem;
		width: 100%;
		text-align: left;
		background: transparent;
		color: inherit;
		border: 0;
		padding: 0.3rem 0.5rem;
		border-radius: 4px;
		cursor: pointer;
		font: inherit;
		font-size: 0.85rem;
	}
	button:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.06);
	}
	:global(.app[data-skin='light']) button:hover:not(:disabled) {
		background: rgba(0, 0, 0, 0.05);
	}
	button.selected {
		background: rgba(246, 196, 84, 0.2);
	}
	button:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}
	.idx {
		font-family: ui-monospace, monospace;
		opacity: 0.55;
		font-variant-numeric: tabular-nums;
	}
	.winner {
		display: flex;
		align-items: center;
		gap: 0.35rem;
	}
	.dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
	}
	.turns {
		font-variant-numeric: tabular-nums;
		opacity: 0.7;
	}
	.flag {
		font-size: 0.7rem;
		opacity: 0.7;
		background: rgba(246, 196, 84, 0.2);
		padding: 0.1rem 0.35rem;
		border-radius: 3px;
	}
	.empty {
		opacity: 0.6;
		font-size: 0.85rem;
		padding: 0.5rem;
	}
</style>
