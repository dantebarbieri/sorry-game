<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';
	import { SimulatorClient } from '$lib/simulate/worker-client.svelte';
	import type { SimConfig } from '$lib/simulate/types';
	import StatsTable from '$lib/components/sim/StatsTable.svelte';
	import GameList from '$lib/components/sim/GameList.svelte';
	import ReplayDrawer from '$lib/components/sim/ReplayDrawer.svelte';
	import { PLAYER_NAMES } from '$lib/play/types';

	interface StrategyDescription {
		name: string;
		description: string;
		complexity: string;
	}

	const client = new SimulatorClient();

	let availableRules = $state<string[]>(['Standard', 'PlayOut']);
	let availableStrategies = $state<StrategyDescription[]>([
		{ name: 'Random', description: 'Picks a legal move uniformly at random.', complexity: 'Trivial' }
	]);

	let numGames = $state(200);
	let rules = $state<'Standard' | 'PlayOut'>('Standard');
	let baseSeed = $state<number>(Math.floor(Math.random() * 1_000_000_000));
	let maxTurns = $state<number | null>(500);
	let keepHistories = $state(true);
	let strategies = $state<string[]>(['Random', 'Random', 'Random', 'Random']);
	let replayIndex = $state<number | null>(null);

	onMount(async () => {
		client.ensureWorker();
		try {
			const wasm = await loadWasm();
			const rulesList = parseJsonOrThrow<string[]>(wasm.get_available_rules());
			if (rulesList.length) availableRules = rulesList;
			const descs = parseJsonOrThrow<StrategyDescription[]>(wasm.get_strategy_descriptions());
			if (descs.length) availableStrategies = descs;
		} catch (e) {
			console.warn('[simulate] metadata load failed', e);
		}
	});

	onDestroy(() => client.dispose());

	function start() {
		const cfg: SimConfig = {
			num_games: numGames,
			rules,
			strategies: strategies.slice(),
			base_seed: baseSeed,
			max_turns: maxTurns ?? undefined,
			keep_histories: keepHistories
		};
		replayIndex = null;
		client.start(cfg);
	}

	function rerollSeed() {
		baseSeed = Math.floor(Math.random() * 1_000_000_000);
	}

	function setStrategy(i: number, name: string) {
		strategies = strategies.map((s, idx) => (idx === i ? name : s));
	}

	const selectedHistory = $derived.by(() => {
		if (replayIndex === null) return null;
		return client.histories?.[replayIndex] ?? null;
	});
</script>

<div class="sim">
	<section class="config">
		<header>
			<h2>Simulator</h2>
			<p class="hint">
				Run games in a Web Worker. Progress updates live; click any completed game to replay it.
			</p>
		</header>
		<div class="grid">
			<label>
				Games
				<input type="number" min="1" max="1000000" step="1" bind:value={numGames} />
			</label>
			<label>
				Rules
				<select bind:value={rules}>
					{#each availableRules as r (r)}
						<option value={r}>{r}</option>
					{/each}
				</select>
			</label>
			<label>
				Seed
				<div class="row">
					<input type="number" bind:value={baseSeed} />
					<button type="button" onclick={rerollSeed} title="New random seed">🎲</button>
				</div>
			</label>
			<label>
				Max turns
				<input
					type="number"
					placeholder="No limit"
					value={maxTurns ?? ''}
					oninput={(e) => {
						const v = (e.currentTarget as HTMLInputElement).value;
						maxTurns = v === '' ? null : Number(v);
					}}
				/>
			</label>
			<label class="check">
				<input type="checkbox" bind:checked={keepHistories} />
				Keep histories (enables per-game replay)
			</label>
		</div>

		<fieldset class="seats">
			<legend>Strategies per seat</legend>
			{#each strategies as strategy, i (i)}
				<label>
					{PLAYER_NAMES[i] ?? `P${i}`}
					<select value={strategy} onchange={(e) => setStrategy(i, (e.currentTarget as HTMLSelectElement).value)}>
						{#each availableStrategies as s (s.name)}
							<option value={s.name} title={s.description}>{s.name}</option>
						{/each}
					</select>
				</label>
			{/each}
		</fieldset>

		<div class="run-controls">
			<button class="primary" onclick={start} disabled={!client.ready || client.running}>
				{client.running ? 'Running…' : 'Start'}
			</button>
			{#if client.running}
				{#if client.paused}
					<button onclick={() => client.resume()}>Resume</button>
				{:else}
					<button onclick={() => client.pause()}>Pause</button>
				{/if}
				<button onclick={() => client.stop()}>Stop</button>
			{/if}
			{#if client.error}
				<span class="err">Error: {client.error}</span>
			{/if}
		</div>

		{#if client.stats}
			<div class="progress">
				<div
					class="bar"
					style:width="{(client.stats.games_played / client.stats.total_games) * 100}%"
				></div>
			</div>
		{/if}
	</section>

	<section class="stats-section">
		<h2>Stats</h2>
		<StatsTable stats={client.stats} strategies={strategies} />
	</section>

	<section class="games-section">
		<GameList
			games={client.games}
			selected={replayIndex}
			onSelect={(i) => (replayIndex = i)}
			canReplay={client.histories !== null}
			numPlayers={strategies.length}
		/>
	</section>
</div>

{#if selectedHistory}
	<ReplayDrawer history={selectedHistory} onClose={() => (replayIndex = null)} />
{/if}

<style>
	.sim {
		display: grid;
		grid-template-columns: minmax(20rem, 28rem) 1fr 18rem;
		gap: 1.25rem;
		padding: 1.5rem;
		max-width: 90rem;
		margin: 0 auto;
		width: 100%;
		align-items: start;
	}
	@media (max-width: 64rem) {
		.sim {
			grid-template-columns: 1fr;
		}
	}
	section {
		background: rgba(255, 255, 255, 0.03);
		border: 1px solid rgba(255, 255, 255, 0.08);
		border-radius: 10px;
		padding: 1rem;
	}
	:global(.app[data-skin='light']) section {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.08);
	}
	h2 {
		margin: 0 0 0.35rem;
		font-size: 1.1rem;
	}
	.hint {
		margin: 0 0 1rem;
		font-size: 0.85rem;
		opacity: 0.7;
	}
	.grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.6rem 0.8rem;
		margin-bottom: 0.75rem;
	}
	label {
		display: flex;
		flex-direction: column;
		font-size: 0.8rem;
		gap: 0.3rem;
		opacity: 0.9;
	}
	label.check {
		grid-column: 1 / -1;
		flex-direction: row;
		align-items: center;
		gap: 0.5rem;
		opacity: 0.85;
	}
	input[type='number'],
	select {
		background: rgba(255, 255, 255, 0.06);
		color: inherit;
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 4px;
		padding: 0.3rem 0.5rem;
		font: inherit;
	}
	:global(.app[data-skin='light']) input[type='number'],
	:global(.app[data-skin='light']) select {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.12);
	}
	.row {
		display: flex;
		gap: 0.3rem;
	}
	.row input {
		flex: 1;
	}
	.seats {
		border: 0;
		padding: 0;
		margin: 0 0 0.75rem;
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.4rem 0.8rem;
	}
	.seats legend {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.65;
		padding: 0;
		margin-bottom: 0.3rem;
	}
	.run-controls {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		flex-wrap: wrap;
	}
	.run-controls button {
		background: transparent;
		color: inherit;
		border: 1px solid currentColor;
		padding: 0.4rem 0.9rem;
		border-radius: 4px;
		font: inherit;
		cursor: pointer;
		opacity: 0.85;
	}
	.run-controls button:hover:not(:disabled) {
		opacity: 1;
	}
	.run-controls button:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}
	.run-controls .primary {
		background: rgba(246, 196, 84, 0.25);
		border-color: rgba(246, 196, 84, 0.7);
		font-weight: 600;
	}
	.err {
		color: salmon;
		font-size: 0.85rem;
	}
	.progress {
		margin-top: 0.75rem;
		height: 4px;
		background: rgba(255, 255, 255, 0.08);
		border-radius: 2px;
		overflow: hidden;
	}
	:global(.app[data-skin='light']) .progress {
		background: rgba(0, 0, 0, 0.1);
	}
	.bar {
		height: 100%;
		background: rgba(246, 196, 84, 0.8);
		transition: width 120ms ease;
	}
</style>
