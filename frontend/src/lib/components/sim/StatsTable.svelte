<script lang="ts">
	import type { ProgressStats } from '$lib/simulate/types';
	import { PLAYER_NAMES, PLACEMENT_ORDINAL } from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';

	interface Props {
		stats: ProgressStats | null;
		strategies: string[];
	}

	const { stats, strategies }: Props = $props();
	const theme = useTheme();

	/** How many placement columns to show: only as many ranks as have any
	 *  data. Standard runs collapse to a single "1st" column; PlayOut runs
	 *  show up to 4. */
	const rankCount = $derived.by(() => {
		if (!stats) return 1;
		let maxFilled = 0;
		for (const row of stats.placements) {
			for (let rank = 0; rank < row.length; rank++) {
				if (row[rank] > 0) maxFilled = Math.max(maxFilled, rank + 1);
			}
		}
		return Math.max(1, maxFilled);
	});
</script>

{#if stats}
	<table class="stats">
		<thead>
			<tr>
				<th scope="col">Seat</th>
				<th scope="col">Strategy</th>
				{#each Array(rankCount) as _, rank (rank)}
					<th scope="col" class="num">{PLACEMENT_ORDINAL[rank] ?? `${rank + 1}th`}</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#each strategies as strategy, p (p)}
				<tr>
					<td>
						<span
							class="dot"
							style:background={theme.skin.palette.players[p]}
							aria-hidden="true"
						></span>
						<span>{PLAYER_NAMES[p] ?? `P${p}`}</span>
					</td>
					<td>{strategy}</td>
					{#each Array(rankCount) as _, rank (rank)}
						<td class="num">
							{((stats.placement_rate[p]?.[rank] ?? 0) * 100).toFixed(1)}%
							<span class="raw">({stats.placements[p]?.[rank] ?? 0})</span>
						</td>
					{/each}
				</tr>
			{/each}
		</tbody>
		<tfoot>
			<tr>
				<td colspan={2 + rankCount}>
					<span class="ftr-label">Games</span>
					<span class="ftr-val">{stats.games_played} / {stats.total_games}</span>
				</td>
			</tr>
			<tr>
				<td colspan={2 + rankCount}>
					<span class="ftr-label">Avg turns</span>
					<span class="ftr-val">{stats.avg_turns.toFixed(1)}</span>
				</td>
			</tr>
			<tr>
				<td colspan={2 + rankCount}>
					<span class="ftr-label">Min / max turns</span>
					<span class="ftr-val">{stats.min_turns} / {stats.max_turns}</span>
				</td>
			</tr>
			<tr>
				<td colspan={2 + rankCount}>
					<span class="ftr-label">Truncated</span>
					<span class="ftr-val">
						{stats.games_played > 0
							? ((stats.truncated_count / stats.games_played) * 100).toFixed(1)
							: '0.0'}%
						<span class="raw">({stats.truncated_count})</span>
					</span>
				</td>
			</tr>
		</tfoot>
	</table>
{:else}
	<p class="empty">No data yet — start a run.</p>
{/if}

<style>
	.stats {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.88rem;
	}
	.stats th,
	.stats td {
		padding: 0.35rem 0.5rem;
		text-align: left;
	}
	.stats thead th {
		font-size: 0.7rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.7;
		border-bottom: 1px solid rgba(255, 255, 255, 0.12);
	}
	:global(.app[data-skin='light']) .stats thead th {
		border-bottom-color: rgba(0, 0, 0, 0.12);
	}
	.stats tbody tr + tr td {
		border-top: 1px solid rgba(255, 255, 255, 0.04);
	}
	.stats tfoot td {
		border-top: 1px solid rgba(255, 255, 255, 0.08);
		opacity: 0.85;
		font-size: 0.85rem;
		display: flex;
		justify-content: space-between;
	}
	:global(.app[data-skin='light']) .stats tfoot td,
	:global(.app[data-skin='light']) .stats tbody tr + tr td {
		border-color: rgba(0, 0, 0, 0.08);
	}
	.num {
		font-variant-numeric: tabular-nums;
		text-align: right;
	}
	.raw {
		opacity: 0.45;
		font-size: 0.78em;
		margin-left: 0.2rem;
	}
	.dot {
		display: inline-block;
		width: 10px;
		height: 10px;
		border-radius: 50%;
		margin-right: 0.45rem;
		vertical-align: baseline;
	}
	.empty {
		opacity: 0.6;
		font-size: 0.9rem;
		padding: 0.5rem;
	}
	.ftr-label {
		opacity: 0.7;
	}
	.ftr-val {
		font-variant-numeric: tabular-nums;
	}
</style>
