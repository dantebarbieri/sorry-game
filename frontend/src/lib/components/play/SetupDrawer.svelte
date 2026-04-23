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
		/** When true, closing without applying is disallowed (first-time setup). */
		required?: boolean;
		onApply: (setup: PlaySetup) => void;
		onClose: () => void;
	}

	const { open, setup, required = false, onApply, onClose }: Props = $props();

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
			seats = Array.from({ length: 4 }, (_, i) =>
				setup.seats[i] ? { ...setup.seats[i] } : { type: 'Bot', strategy: 'Random' }
			);
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

	function tryClose() {
		if (!required) onClose();
	}

	const activeCount = $derived(seats.filter((s) => s.type !== 'Empty').length);
	const humanCount = $derived(seats.filter((s) => s.type === 'Human').length);
	const canApply = $derived(activeCount >= 2);

	function apply() {
		if (!canApply) return;
		onApply({ rules, seats });
	}
</script>

{#if open}
	<div
		class="scrim"
		role="presentation"
		onclick={tryClose}
		onkeydown={(e) => e.key === 'Escape' && tryClose()}
	></div>
	<div class="drawer" role="dialog" aria-modal="true" aria-label="New game setup">
		<header>
			<h2>{required ? 'Configure game' : 'New game'}</h2>
			{#if !required}
				<button class="close" onclick={onClose} aria-label="Close setup">✕</button>
			{/if}
		</header>

		<div class="section">
			<label class="block">
				<span class="section-title">Rules</span>
				<select bind:value={rules}>
					{#each availableRules as r (r)}
						<option value={r}>{r}</option>
					{/each}
				</select>
			</label>
			<p class="hint">
				{#if rules === 'Standard'}
					Standard — first player to land all four pawns home wins.
				{:else if rules === 'PlayOut'}
					Play Out — play continues until every placement (1st→4th) is decided.
				{/if}
			</p>
		</div>

		<div class="section">
			<span class="section-title">Players</span>
			<p class="hint">
				Each color is Human, a bot strategy, or Empty (that color sits out).
				Pick your favorite color to play as. Minimum 2 players.
			</p>
			<div class="seats">
				{#each seats as seat, i (i)}
					<div class="seat" class:empty={seat.type === 'Empty'}>
						<span
							class="badge"
							style:background={theme.skin.palette.players[i]}
							aria-hidden="true"
						></span>
						<span class="seat-name">{PLAYER_NAMES[i]}</span>
						<div class="seat-controls">
							<select
								aria-label="{PLAYER_NAMES[i]} seat type"
								value={seat.type === 'Bot' ? `Bot:${seat.strategy}` : seat.type}
								onchange={(e) => {
									const val = (e.currentTarget as HTMLSelectElement).value;
									if (val === 'Human') setSeat(i, { type: 'Human' });
									else if (val === 'Empty') setSeat(i, { type: 'Empty' });
									else if (val.startsWith('Bot:'))
										setSeat(i, { type: 'Bot', strategy: val.slice(4) });
								}}
							>
								<option value="Human">Human</option>
								<option value="Empty">Not playing</option>
								{#each availableStrategies as s (s.name)}
									<option value={'Bot:' + s.name} title={s.description}>Bot · {s.name}</option>
								{/each}
							</select>
						</div>
					</div>
				{/each}
			</div>
			{#if activeCount < 2}
				<p class="error">At least two colors must be playing.</p>
			{:else if humanCount === 0}
				<p class="hint">No humans configured — you'll be an observer.</p>
			{/if}
		</div>

		<footer>
			{#if !required}
				<button class="secondary" onclick={onClose}>Cancel</button>
			{/if}
			<button class="primary" onclick={apply} disabled={!canApply}>Start game</button>
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
	.section-title {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		opacity: 0.65;
	}
	.block {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
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
	.error {
		font-size: 0.8rem;
		color: salmon;
		margin: 0.2rem 0 0;
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
	.seat.empty {
		opacity: 0.55;
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
		gap: 0.5rem;
		justify-self: end;
		min-width: 10rem;
	}
	.seat-controls select {
		flex: 1;
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
	.primary:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.secondary {
		background: transparent;
		color: inherit;
		border: 1px solid currentColor;
		opacity: 0.8;
	}
</style>
