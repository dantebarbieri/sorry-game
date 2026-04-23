<script lang="ts">
	import { onMount } from 'svelte';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';
	import type { PlaySetup, SeatKind, Ruleset } from '$lib/play/types';
	import { PLAYER_NAMES } from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';

	interface StrategyDescription {
		name: string;
		description: string;
		complexity: string;
	}

	interface Props {
		open: boolean;
		setup: PlaySetup;
		onApply: (setup: PlaySetup) => void;
		onClose: () => void;
	}

	const { open, setup, onApply, onClose }: Props = $props();

	const theme = useTheme();

	let rules = $state<Ruleset>('Standard');
	let seats = $state<SeatKind[]>([]);
	let availableRules = $state<string[]>(['Standard', 'PlayOut']);
	let availableStrategies = $state<StrategyDescription[]>([
		{ name: 'Random', description: 'Picks a legal move uniformly at random.', complexity: 'Trivial' }
	]);

	// Keep local state in sync when the parent reopens with a different setup.
	$effect(() => {
		if (open) {
			rules = setup.rules;
			seats = setup.seats.map((s) => ({ ...s }));
		}
	});

	onMount(async () => {
		try {
			const wasm = await loadWasm();
			availableRules = parseJsonOrThrow<string[]>(wasm.get_available_rules());
			availableStrategies = parseJsonOrThrow<StrategyDescription[]>(
				wasm.get_strategy_descriptions()
			);
		} catch (e) {
			console.warn('[SetupDrawer] failed to load metadata', e);
		}
	});

	function setSeat(i: number, kind: SeatKind) {
		seats = seats.map((s, idx) => (idx === i ? kind : s));
	}

	function apply() {
		onApply({ rules, seats });
	}

	function preset(name: 'solo-vs-bots' | 'observer' | 'hotseat') {
		if (name === 'solo-vs-bots') {
			seats = [
				{ type: 'Human' },
				{ type: 'Bot', strategy: 'Random' },
				{ type: 'Bot', strategy: 'Random' },
				{ type: 'Bot', strategy: 'Random' }
			];
		} else if (name === 'observer') {
			seats = [
				{ type: 'Bot', strategy: 'Random' },
				{ type: 'Bot', strategy: 'Random' },
				{ type: 'Bot', strategy: 'Random' },
				{ type: 'Bot', strategy: 'Random' }
			];
		} else {
			seats = [
				{ type: 'Human' },
				{ type: 'Human' },
				{ type: 'Human' },
				{ type: 'Human' }
			];
		}
	}
</script>

{#if open}
	<div
		class="scrim"
		role="presentation"
		onclick={onClose}
		onkeydown={(e) => e.key === 'Escape' && onClose()}
	></div>
	<div class="drawer" role="dialog" aria-modal="true" aria-label="New game setup">
		<header>
			<h2>New game</h2>
			<button class="close" onclick={onClose} aria-label="Close setup">✕</button>
		</header>

		<div class="section">
			<label for="rules-select">Rules</label>
			<select id="rules-select" bind:value={rules}>
				{#each availableRules as r (r)}
					<option value={r}>{r}</option>
				{/each}
			</select>
			<p class="hint">
				{#if rules === 'Standard'}
					Standard — first player to land all four pawns home wins.
				{:else if rules === 'PlayOut'}
					Play Out — play continues until every placement (1st→4th) is decided.
				{/if}
			</p>
		</div>

		<div class="section">
			<span class="section-title">Presets</span>
			<div class="preset-row">
				<button type="button" onclick={() => preset('solo-vs-bots')}>Solo vs. bots</button>
				<button type="button" onclick={() => preset('hotseat')}>Hot-seat</button>
				<button type="button" onclick={() => preset('observer')}>Observer</button>
			</div>
		</div>

		<div class="section">
			<span class="section-title">Seats</span>
			<div class="seats">
				{#each seats as seat, i (i)}
					<div class="seat">
						<span
							class="badge"
							style:background={theme.skin.palette.players[i]}
							aria-hidden="true"
						></span>
						<span class="seat-name">{PLAYER_NAMES[i]}</span>
						<div class="seat-controls">
							<label class="radio">
								<input
									type="radio"
									name="seat-{i}"
									checked={seat.type === 'Human'}
									onchange={() => setSeat(i, { type: 'Human' })}
								/>
								Human
							</label>
							<label class="radio">
								<input
									type="radio"
									name="seat-{i}"
									checked={seat.type === 'Bot'}
									onchange={() =>
										setSeat(i, {
											type: 'Bot',
											strategy: seat.type === 'Bot' ? seat.strategy : 'Random'
										})
									}
								/>
								Bot
							</label>
							{#if seat.type === 'Bot'}
								<select
									value={seat.strategy}
									onchange={(e) =>
										setSeat(i, {
											type: 'Bot',
											strategy: (e.currentTarget as HTMLSelectElement).value
										})}
									aria-label="Strategy for {PLAYER_NAMES[i]}"
								>
									{#each availableStrategies as s (s.name)}
										<option value={s.name} title={s.description}>{s.name}</option>
									{/each}
								</select>
							{/if}
						</div>
					</div>
				{/each}
			</div>
			<p class="hint">
				More strategies coming soon — see the
				<a href="/rules#strategies">Strategies</a> section of the rules for planned bots.
			</p>
		</div>

		<footer>
			<button class="secondary" onclick={onClose}>Cancel</button>
			<button class="primary" onclick={apply}>Start game</button>
		</footer>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.45);
		z-index: 100;
	}
	.drawer {
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(28rem, 96vw);
		background: #141c28;
		color: #e8ecf2;
		padding: 1rem 1.25rem;
		overflow-y: auto;
		z-index: 101;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		border-left: 1px solid rgba(255, 255, 255, 0.08);
		box-shadow: -12px 0 32px rgba(0, 0, 0, 0.4);
	}
	:global(.app[data-skin='light']) .drawer {
		background: #faf4e0;
		color: #23201a;
		border-left-color: rgba(0, 0, 0, 0.08);
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	h2 {
		margin: 0;
		font-size: 1.1rem;
	}
	.close {
		background: transparent;
		border: 0;
		color: inherit;
		font-size: 1.2rem;
		cursor: pointer;
		padding: 0.2rem 0.5rem;
	}
	.section {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}
	.section > label,
	.section > .section-title {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		opacity: 0.65;
	}
	select {
		background: rgba(255, 255, 255, 0.06);
		color: inherit;
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 4px;
		padding: 0.3rem 0.5rem;
		font: inherit;
	}
	:global(.app[data-skin='light']) select {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.12);
	}
	.hint {
		font-size: 0.8rem;
		opacity: 0.7;
		margin: 0;
	}
	.hint a {
		color: inherit;
	}
	.preset-row {
		display: flex;
		gap: 0.4rem;
		flex-wrap: wrap;
	}
	.preset-row button {
		background: rgba(255, 255, 255, 0.06);
		border: 1px solid rgba(255, 255, 255, 0.12);
		color: inherit;
		padding: 0.3rem 0.7rem;
		border-radius: 4px;
		cursor: pointer;
		font: inherit;
	}
	:global(.app[data-skin='light']) .preset-row button {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.12);
	}
	.seats {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.seat {
		display: grid;
		grid-template-columns: auto auto 1fr;
		gap: 0.7rem;
		align-items: center;
		padding: 0.5rem;
		border-radius: 6px;
		background: rgba(255, 255, 255, 0.03);
	}
	:global(.app[data-skin='light']) .seat {
		background: rgba(0, 0, 0, 0.03);
	}
	.badge {
		width: 14px;
		height: 14px;
		border-radius: 50%;
	}
	.seat-name {
		font-weight: 600;
		min-width: 3.5rem;
	}
	.seat-controls {
		display: flex;
		align-items: center;
		gap: 0.65rem;
		flex-wrap: wrap;
		justify-self: end;
	}
	.radio {
		display: inline-flex;
		gap: 0.25rem;
		align-items: center;
		font-size: 0.9rem;
		cursor: pointer;
	}
	footer {
		margin-top: auto;
		display: flex;
		gap: 0.5rem;
		justify-content: flex-end;
	}
	.primary,
	.secondary {
		padding: 0.5rem 1rem;
		border-radius: 6px;
		border: 0;
		font: inherit;
		cursor: pointer;
	}
	.primary {
		background: #f6c454;
		color: #1b1710;
		font-weight: 600;
	}
	.secondary {
		background: transparent;
		color: inherit;
		border: 1px solid currentColor;
		opacity: 0.8;
	}
</style>
