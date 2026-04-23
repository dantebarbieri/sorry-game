<script lang="ts">
	import { onMount } from 'svelte';

	import BoardCanvas, {
		type CameraCommand,
		type HighlightState,
		type StepCommand
	} from '$lib/board/BoardCanvas.svelte';
	import type { BoardGeometry } from '$lib/board/geometry';
	import { LIGHT_SKIN, DARK_SKIN } from '$lib/board/skins';
	import type { CameraView } from '$lib/board/renderer';
	import type { GameHistory } from '$lib/board/actions';
	import { cardLabel } from '$lib/board/cards';
	import { describeAction } from '$lib/board/a11y';
	import { HistoryCursor, type PlayBeat } from '$lib/board/history-cursor';
	import type { ReplayState } from '$lib/board/replay';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';

	interface SimResponse {
		history: GameHistory;
	}

	const PLAYER_NAMES = ['Red', 'Blue', 'Yellow', 'Green'];
	const PLACEMENT_ORDINAL = ['1st', '2nd', '3rd', '4th'];
	type Ruleset = 'Standard' | 'PlayOut';

	let geometry = $state<BoardGeometry | null>(null);
	let history = $state<GameHistory | null>(null);
	let cursor = $state<HistoryCursor | null>(null);
	let viewState = $state<ReplayState | null>(null);
	let lastStep = $state<StepCommand | undefined>(undefined);
	let lastBeat = $state<PlayBeat | null>(null);
	let cursorIndex = $state(0);
	let cursorLength = $state(0);
	let animating = $state(false);
	let autoPlay = $state(false);
	let skinName = $state<'light' | 'dark'>('light');
	let activePreset = $state<CameraView | null>('corner');
	let cameraCommand = $state<CameraCommand | undefined>(undefined);
	let ruleset = $state<Ruleset>('Standard');
	let seed = $state<number>(Math.floor(Math.random() * 1_000_000_000));
	let lastAnnouncement = $state('');
	let error = $state<string | null>(null);
	let loading = $state(true);
	// Per-seat strategy selection for generating a new random game. Today
	// the engine ships only "Random", so all four default to that — the
	// state + UI are here to accept future strategies (Greedy, MCTS, etc.)
	// without a layout rewrite.
	let availableStrategies = $state<string[]>(['Random']);
	let seatStrategies = $state<string[]>(['Random', 'Random', 'Random', 'Random']);
	let setupOpen = $state(false);
	let importError = $state<string | null>(null);
	let fileInput: HTMLInputElement | undefined = $state();

	async function ensureGeometry() {
		if (geometry) return geometry;
		const wasm = await loadWasm();
		geometry = parseJsonOrThrow<BoardGeometry>(wasm.get_board_geometry('Standard'));
		return geometry;
	}

	async function ensureAvailableStrategies() {
		try {
			const wasm = await loadWasm();
			const names = parseJsonOrThrow<string[]>(wasm.get_available_strategies());
			if (Array.isArray(names) && names.length > 0) availableStrategies = names;
		} catch {
			/* keep ['Random'] fallback */
		}
	}

	async function loadReplay() {
		loading = true;
		error = null;
		try {
			const wasm = await loadWasm();
			await ensureGeometry();
			const res = parseJsonOrThrow<SimResponse>(
				wasm.simulate_one_with_history(
					JSON.stringify({
						seed,
						strategies: seatStrategies,
						rules: ruleset
					})
				)
			);
			adoptHistory(res.history);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	function adoptHistory(h: GameHistory) {
		const geom = geometry;
		if (!geom) throw new Error('geometry must be loaded before adopting a history');
		history = h;
		const c = new HistoryCursor(h, geom);
		cursor = c;
		cursorIndex = c.index;
		cursorLength = c.length;
		viewState = c.currentState;
		lastStep = undefined;
		lastBeat = null;
		lastAnnouncement = '';
		autoPlay = false;
		animating = false;
	}

	async function importHistoryFile(file: File) {
		importError = null;
		loading = true;
		try {
			await ensureGeometry();
			const text = await file.text();
			const parsed = JSON.parse(text) as GameHistory;
			if (!Array.isArray(parsed.turns) || !Array.isArray(parsed.initial_deck_order)) {
				throw new Error('JSON is missing required GameHistory fields (turns, initial_deck_order)');
			}
			// Sync ruleset + seed so the footer reflects the imported game.
			if (parsed.rules_name === 'PlayOut' || parsed.rules_name === 'Standard') {
				ruleset = parsed.rules_name;
			}
			if (typeof parsed.seed === 'number') seed = parsed.seed;
			adoptHistory(parsed);
		} catch (e) {
			importError = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	function onImportInput(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (file) void importHistoryFile(file);
		input.value = ''; // allow re-picking the same file
	}

	function exportHistory() {
		if (!history) return;
		const blob = new Blob([JSON.stringify(history, null, 2)], { type: 'application/json' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `sorry-${history.rules_name}-seed${history.seed}.json`;
		a.click();
		URL.revokeObjectURL(url);
	}

	function stepForward() {
		if (!cursor || animating) return;
		const beat = cursor.stepForward();
		if (!beat) return;
		animating = true;
		lastBeat = beat;
		lastStep = {
			record: beat.record,
			player: beat.player,
			nonce: (lastStep?.nonce ?? 0) + 1
		};
		viewState = beat.next;
		cursorIndex = cursor.index;
	}

	function stepBack() {
		if (!cursor) return;
		const state = cursor.stepBack();
		if (!state) return;
		snapTo(state);
	}

	function jumpTo(index: number) {
		if (!cursor) return;
		snapTo(cursor.jumpTo(index));
	}

	/**
	 * Apply a state as an instant snap (no animation). Cancels any
	 * in-flight step animation by clearing `lastStep` so the renderer's
	 * `setState` goes down the no-record path (animator.cancel +
	 * snapPawns) instead of queuing on top of whatever was playing.
	 * Also resets `animating` since the canceled animation's onDone
	 * won't fire, and pauses auto-play so a scrub doesn't keep running
	 * forward from the new position.
	 */
	function snapTo(state: ReplayState) {
		viewState = state;
		cursorIndex = cursor!.index;
		lastStep = undefined;
		lastBeat = null;
		lastAnnouncement = '';
		animating = false;
		autoPlay = false;
	}

	function onStepEnd(_step: StepCommand) {
		animating = false;
		const beat = lastBeat;
		if (beat && geometry) {
			lastAnnouncement = describeAction(beat.record, beat.player, geometry);
		}
		if (autoPlay) {
			if (cursor && !cursor.isAtEnd) stepForward();
			else autoPlay = false;
		}
	}

	$effect(() => {
		if (autoPlay && cursor && !cursor.isAtEnd && !animating) stepForward();
	});

	function toggleAutoPlay() {
		autoPlay = !autoPlay;
	}

	function newGame(r: Ruleset) {
		ruleset = r;
		seed = Math.floor(Math.random() * 1_000_000_000);
		void loadReplay();
	}

	function pickView(view: CameraView) {
		activePreset = view;
		cameraCommand = { view, nonce: (cameraCommand?.nonce ?? 0) + 1 };
	}

	function onUserOrbit() {
		activePreset = null;
	}

	function onKeyDown(e: KeyboardEvent) {
		const target = e.target as Element | null;
		if (target && (target.tagName === 'BUTTON' || target.tagName === 'INPUT')) return;
		if (e.key === 'ArrowRight') {
			e.preventDefault();
			stepForward();
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			stepBack();
		} else if (e.key === 'Home') {
			e.preventDefault();
			jumpTo(0);
		} else if (e.key === 'End' && cursor) {
			e.preventDefault();
			jumpTo(cursor.length);
		} else if (e.key === ' ') {
			e.preventDefault();
			toggleAutoPlay();
		}
	}

	onMount(() => {
		void ensureAvailableStrategies();
		void loadReplay();
	});

	const currentSkin = $derived(skinName === 'light' ? LIGHT_SKIN : DARK_SKIN);
	const highlights = $derived.by<HighlightState>(() => ({
		destinations: [],
		selectedPawn: null,
		currentPlayer: viewState?.current_player ?? 0
	}));
	const beatLabel = $derived.by(() => {
		if (!lastBeat) return null;
		const card = cardLabel(lastBeat.record.card) ?? lastBeat.record.card;
		return {
			label: card,
			color: currentSkin.palette.players[lastBeat.player],
			name: PLAYER_NAMES[lastBeat.player] ?? `P${lastBeat.player}`
		};
	});
	/** True once the replay cursor is at the very end of the history. In
	 *  Play Out this is when the last-place finisher has been appended
	 *  by `finalize_winners`; in Standard it's the instant the winner
	 *  lands their last pawn.
	 *
	 *  Reads `cursorIndex` / `cursorLength` (both `$state`) rather than
	 *  the cursor instance's getters, since Svelte 5 doesn't track plain
	 *  class properties as reactive. */
	const isAtGameEnd = $derived.by(() => {
		if (!history || cursorLength === 0) return false;
		return (
			cursorIndex >= cursorLength && (history.winners.length > 0 || history.truncated)
		);
	});

	/** Winners to render in the placements panel. While mid-replay, use
	 *  viewState's progressively-built list (only truly-home finishers).
	 *  At the end, switch to history.winners so Play Out's last-place
	 *  (appended by `finalize_winners`) appears too. */
	const displayWinners = $derived.by<number[]>(() => {
		if (isAtGameEnd && history) return history.winners;
		return viewState?.winners ?? [];
	});

	const placementsTitle = $derived(isAtGameEnd ? 'Final placement' : 'Placements');

	const truncatedAtEnd = $derived.by(() => {
		return isAtGameEnd && history?.truncated === true && history.winners.length === 0;
	});
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="replay">
	<header>
		<div class="lhs">
			<h1>Replay</h1>
			{#if beatLabel}
				<span class="card-chip" style:color={beatLabel.color}>{beatLabel.label}</span>
				<span class="move-name" style:color={beatLabel.color}>{beatLabel.name}</span>
			{/if}
			{#if cursor}
				<span class="progress">
					Play {Math.min(cursorIndex + 1, cursorLength)} / {cursorLength}
				</span>
			{/if}
		</div>
		<div class="controls">
			<div class="group" aria-label="Skin">
				<button onclick={() => (skinName = 'light')} class:active={skinName === 'light'}>
					Light
				</button>
				<button onclick={() => (skinName = 'dark')} class:active={skinName === 'dark'}>
					Dark
				</button>
			</div>
			<div class="group" aria-label="New game">
				<button onclick={() => newGame('Standard')} title="Generate a new Standard-rules game">
					New Standard
				</button>
				<button
					onclick={() => newGame('PlayOut')}
					title="Generate a new Play-Out game (continues after first finisher to establish 1st→4th)"
				>
					New Play Out
				</button>
				<button
					onclick={() => (setupOpen = !setupOpen)}
					class:active={setupOpen}
					aria-pressed={setupOpen}
					aria-expanded={setupOpen}
					title="Configure per-seat strategies and import/export history"
				>
					Setup
				</button>
			</div>
			<div class="group" aria-label="Camera view">
				<button onclick={() => pickView('edge')} class:active={activePreset === 'edge'}>
					Edge
				</button>
				<button onclick={() => pickView('corner')} class:active={activePreset === 'corner'}>
					Corner
				</button>
				<button onclick={() => pickView('top')} class:active={activePreset === 'top'}>
					Top
				</button>
			</div>
			<div class="group" aria-label="Playback">
				<button onclick={() => jumpTo(0)} disabled={!cursor || cursorIndex === 0}>
					⏮
				</button>
				<button onclick={stepBack} disabled={!cursor || cursorIndex === 0}>
					|◀
				</button>
				<button
					onclick={toggleAutoPlay}
					class:active={autoPlay}
					disabled={!cursor || cursor.isAtEnd}
					aria-pressed={autoPlay}
				>
					{autoPlay ? '⏸' : '▶'}
				</button>
				<button
					onclick={stepForward}
					disabled={!cursor || cursor.isAtEnd || animating}
				>
					▶|
				</button>
				<button
					onclick={() => cursor && jumpTo(cursor.length)}
					disabled={!cursor || cursor.isAtEnd}
				>
					⏭
				</button>
			</div>
		</div>
	</header>
	{#if setupOpen}
		<div class="setup" role="region" aria-label="Replay setup">
			<div class="setup-row">
				<span class="setup-label">Strategies</span>
				{#each [0, 1, 2, 3] as seat (seat)}
					<div class="seat-picker">
						<span class="seat-name" style:color={currentSkin.palette.players[seat]}>
							{PLAYER_NAMES[seat]}
						</span>
						<select
							value={seatStrategies[seat]}
							onchange={(e) =>
								(seatStrategies[seat] = (e.currentTarget as HTMLSelectElement).value)}
							aria-label={`Strategy for ${PLAYER_NAMES[seat]}`}
						>
							{#each availableStrategies as name (name)}
								<option value={name}>{name}</option>
							{/each}
						</select>
					</div>
				{/each}
			</div>
			<div class="setup-row">
				<span class="setup-label">History</span>
				<input
					bind:this={fileInput}
					type="file"
					accept="application/json,.json"
					onchange={onImportInput}
					class="hidden-file"
				/>
				<button onclick={() => fileInput?.click()} title="Import a saved GameHistory JSON">
					Import…
				</button>
				<button
					onclick={exportHistory}
					disabled={!history}
					title="Download the current history as JSON"
				>
					Export
				</button>
				{#if importError}
					<span class="import-error">Import failed: {importError}</span>
				{/if}
			</div>
		</div>
	{/if}
	<div class="canvas-wrap">
		{#if error}
			<p class="msg error">Error: {error}</p>
		{:else if loading || !geometry || !viewState}
			<p class="msg">Simulating…</p>
		{:else}
			<BoardCanvas
				{geometry}
				skin={currentSkin}
				gameState={viewState}
				{lastStep}
				{cameraCommand}
				{highlights}
				mode="replay"
				{onUserOrbit}
				onStepEnd={onStepEnd}
			/>
		{/if}
		{#if displayWinners.length > 0}
			<div class="placements" role="status" aria-live="polite" aria-label={placementsTitle}>
				<h2>{placementsTitle}</h2>
				<ol>
					{#each displayWinners as player, i (player)}
						<li style:color={currentSkin.palette.players[player] ?? '#e8ecf2'}>
							<span class="ordinal">{PLACEMENT_ORDINAL[i] ?? `${i + 1}th`}</span>
							<span class="name">{PLAYER_NAMES[player] ?? `P${player}`}</span>
						</li>
					{/each}
				</ol>
			</div>
		{:else if truncatedAtEnd}
			<div class="placements" role="status" aria-live="polite">
				<h2>Truncated</h2>
				<p>The game hit the turn limit before any player could finish.</p>
			</div>
		{/if}
	</div>
	<footer>
		<input
			type="range"
			min="0"
			max={cursorLength}
			value={cursorIndex}
			disabled={!cursor}
			aria-label="Scrub to play"
			oninput={(e) => jumpTo(Number((e.target as HTMLInputElement).value))}
		/>
		{#if history}
			<small>
				{history.rules_name} · seed {history.seed} · {history.num_players} players ·
				{history.turns.length} turns · {cursorLength} plays
			</small>
		{/if}
	</footer>

	<div class="visually-hidden" aria-live="polite" aria-atomic="true">
		{lastAnnouncement}
	</div>
</div>

<style>
	:global(html),
	:global(body) {
		margin: 0;
		padding: 0;
		height: 100%;
		overflow: hidden;
	}
	.replay {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		background: #0e141c;
		color: #e8ecf2;
		font-family: system-ui, sans-serif;
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.4rem 0.8rem;
		background: #131c27;
		border-bottom: 1px solid #1d2835;
	}
	h1 {
		font-size: 1rem;
		font-weight: 600;
		margin: 0;
	}
	.lhs {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}
	.card-chip {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 1.8rem;
		padding: 0.1rem 0.4rem;
		border-radius: 0.35rem;
		background: #0a1220;
		border: 1px solid currentColor;
		font-weight: 700;
		font-size: 0.9rem;
	}
	.move-name {
		font-size: 0.85rem;
		font-weight: 500;
	}
	.progress {
		font-size: 0.85rem;
		color: #98a4b4;
	}
	.controls {
		display: flex;
		gap: 0.5rem;
	}
	.group {
		display: flex;
		gap: 0.25rem;
		padding: 0.1rem;
		border: 1px solid #243042;
		border-radius: 0.3rem;
	}
	button {
		background: #1a2432;
		color: #e8ecf2;
		border: 1px solid transparent;
		border-radius: 0.25rem;
		padding: 0.25rem 0.6rem;
		font-size: 0.85rem;
		cursor: pointer;
	}
	button:hover:not(:disabled) {
		background: #243243;
	}
	button:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}
	button.active {
		background: #2d5b9e;
		border-color: #4d7ec6;
	}
	.canvas-wrap {
		flex: 1;
		min-height: 0;
		position: relative;
	}
	.setup {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0.5rem 0.8rem;
		background: #101821;
		border-bottom: 1px solid #1d2835;
		font-size: 0.85rem;
	}
	.setup-row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		flex-wrap: wrap;
	}
	.setup-label {
		color: #98a4b4;
		font-weight: 600;
		min-width: 5rem;
	}
	.seat-picker {
		display: flex;
		align-items: center;
		gap: 0.3rem;
	}
	.seat-name {
		font-weight: 600;
		min-width: 3.5rem;
	}
	.seat-picker select {
		background: #1a2432;
		color: #e8ecf2;
		border: 1px solid #243042;
		border-radius: 0.25rem;
		padding: 0.15rem 0.4rem;
		font-size: 0.85rem;
	}
	.hidden-file {
		display: none;
	}
	.import-error {
		color: #ff6b6b;
	}
	.msg {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		margin: 0;
	}
	.error {
		color: #ff6b6b;
	}
	.placements {
		position: absolute;
		top: 0.75rem;
		right: 0.75rem;
		background: rgba(14, 20, 28, 0.88);
		border: 1px solid #334258;
		border-radius: 0.4rem;
		padding: 0.55rem 0.8rem;
		font-size: 0.85rem;
		min-width: 9rem;
		pointer-events: none;
	}
	.placements h2 {
		margin: 0 0 0.35rem;
		font-size: 0.8rem;
		font-weight: 600;
		color: #98a4b4;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.placements ol {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}
	.placements li {
		display: flex;
		gap: 0.4rem;
		font-weight: 600;
	}
	.placements p {
		margin: 0;
		font-size: 0.8rem;
	}
	.ordinal {
		opacity: 0.7;
	}
	footer {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.35rem 0.8rem;
		background: #131c27;
		border-top: 1px solid #1d2835;
	}
	footer input[type='range'] {
		flex: 1;
	}
	footer small {
		color: #98a4b4;
		font-size: 0.75rem;
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
