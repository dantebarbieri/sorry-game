<script lang="ts">
	import type { ProgressStats } from '$lib/simulate/types';
	import { PLAYER_NAMES } from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';

	interface Props {
		stats: ProgressStats | null;
		strategies: string[];
	}

	const { stats, strategies }: Props = $props();
	const theme = useTheme();
</script>

{#if stats}
	<table class="stats">
		<thead>
			<tr>
				<th scope="col">Seat</th>
				<th scope="col">Strategy</th>
				<th scope="col">Wins</th>
				<th scope="col">Win rate</th>
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
					<td class="num">{stats.wins[p] ?? 0}</td>
					<td class="num">{((stats.win_rate[p] ?? 0) * 100).toFixed(1)}%</td>
				</tr>
			{/each}
		</tbody>
		<tfoot>
			<tr>
				<td colspan="2">Games</td>
				<td class="num" colspan="2">
					{stats.games_played} / {stats.total_games}
				</td>
			</tr>
			<tr>
				<td colspan="2">Avg turns</td>
				<td class="num" colspan="2">{stats.avg_turns.toFixed(1)}</td>
			</tr>
			<tr>
				<td colspan="2">Min / max turns</td>
				<td class="num" colspan="2">{stats.min_turns} / {stats.max_turns}</td>
			</tr>
			<tr>
				<td colspan="2">Truncated</td>
				<td class="num" colspan="2">{stats.truncated_count}</td>
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
		font-size: 0.9rem;
	}
	.stats th,
	.stats td {
		padding: 0.35rem 0.6rem;
		text-align: left;
	}
	.stats thead th {
		font-size: 0.72rem;
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
	}
	:global(.app[data-skin='light']) .stats tfoot td,
	:global(.app[data-skin='light']) .stats tbody tr + tr td {
		border-color: rgba(0, 0, 0, 0.08);
	}
	.num {
		font-variant-numeric: tabular-nums;
		text-align: right;
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
</style>
