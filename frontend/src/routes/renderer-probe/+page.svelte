<script lang="ts">
	import { onMount } from 'svelte';

	import BoardCanvas, {
		type CameraCommand,
		type StepCommand
	} from '$lib/board/BoardCanvas.svelte';
	import type { BoardGeometry } from '$lib/board/geometry';
	import { LIGHT_SKIN, DARK_SKIN } from '$lib/board/skins';
	import type { GameStateView } from '$lib/board/state';
	import type { CameraView } from '$lib/board/renderer';
	import { findLastPlay, type GameHistory } from '$lib/board/actions';
	import { PIECE_MODEL_ATTRIBUTION } from '$lib/board/assets';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';

	interface CreateResponse {
		game_id: number;
		state: GameStateView;
	}
	interface BotActionResponse {
		action: unknown;
		state: GameStateView;
	}

	interface LastPlayed {
		player: number;
		card: string;
	}

	// Maps `Card` enum names to their displayed short label.
	const CARD_LABEL: Record<string, string> = {
		One: '1',
		Two: '2',
		Three: '3',
		Four: '4',
		Five: '5',
		Seven: '7',
		Eight: '8',
		Ten: '10',
		Eleven: '11',
		Twelve: '12',
		Sorry: 'Sorry!'
	};

	let geometry = $state<BoardGeometry | null>(null);
	let gameId = $state<number | null>(null);
	let gameState = $state<GameStateView | null>(null);
	let lastStep = $state<StepCommand | undefined>(undefined);
	let lastPlayed = $state<LastPlayed | null>(null);
	let stepping = $state(false);
	let skinName = $state<'light' | 'dark'>('light');
	let activePreset = $state<CameraView | null>(null);
	let cameraCommand = $state<CameraCommand | undefined>(undefined);
	let error = $state<string | null>(null);

	async function startGame() {
		try {
			const wasm = await loadWasm();
			if (!geometry) {
				geometry = parseJsonOrThrow<BoardGeometry>(wasm.get_board_geometry('Standard'));
			}
			if (gameId !== null) wasm.destroy_interactive_game(gameId);
			const created = parseJsonOrThrow<CreateResponse>(
				wasm.create_interactive_game(
					JSON.stringify({
						strategy_names: ['Random', 'Random', 'Random', 'Random'],
						seed: Math.floor(Math.random() * 1_000_000_000),
						rules: 'Standard'
					})
				)
			);
			gameId = created.game_id;
			gameState = created.state;
			lastStep = undefined;
			lastPlayed = null;
			error = null;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function stepBot() {
		if (gameId === null || stepping) return;
		stepping = true;
		try {
			const wasm = await loadWasm();
			const resp = parseJsonOrThrow<BotActionResponse>(
				wasm.apply_bot_action(gameId, 'Random')
			);
			// Pull the last `Play` from history for its bumps + slides payload
			// (the top-level PlayerAction carries the move but not consequences).
			const history = parseJsonOrThrow<GameHistory>(wasm.get_game_history(gameId));
			// `resp.state.current_turn` holds the in-progress turn (including
			// a just-played 2 that's still mid-extra-turn-chain). findLastPlay
			// will prefer it over the finalized history.
			const play = findLastPlay(history, resp.state.current_turn);
			gameState = resp.state;
			if (play) {
				lastStep = {
					record: play.record,
					player: play.player,
					nonce: (lastStep?.nonce ?? 0) + 1
				};
				// Don't touch `lastPlayed` here — it updates from the
				// `onStepStart` callback so the chip stays in sync with
				// whichever step the animator is currently playing (not
				// the most recently-queued).
			}
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			stepping = false;
		}
	}

	function onStepStart(step: StepCommand) {
		lastPlayed = { player: step.player, card: step.record.card };
	}

	function pickView(view: CameraView) {
		activePreset = view;
		cameraCommand = { view, nonce: (cameraCommand?.nonce ?? 0) + 1 };
	}

	function onUserOrbit() {
		activePreset = null;
	}

	onMount(() => {
		void startGame();
	});

	const currentSkin = $derived(skinName === 'light' ? LIGHT_SKIN : DARK_SKIN);
	const gameOver = $derived.by(() => {
		const s = gameState;
		if (!s) return false;
		return s.winners.length > 0 || s.truncated;
	});
	const turnLabel = $derived.by(() => {
		const s = gameState;
		if (!s) return '';
		if (s.winners.length > 0) return `Game over — P${s.winners[0]} wins`;
		if (s.truncated) return 'Game over (truncated)';
		return `Turn ${s.turn_count + 1} · P${s.current_player}`;
	});
	const lastPlayedLabel = $derived.by(() => {
		const lp = lastPlayed;
		if (!lp) return null;
		return { label: CARD_LABEL[lp.card] ?? lp.card, color: currentSkin.palette.players[lp.player] };
	});
	const lastMoveDesc = $derived.by(() => {
		const step = lastStep;
		if (!step) return null;
		const mv = step.record.mv;
		switch (mv.type) {
			case 'Advance':
				return `Advance pawn ${mv.pawn} → ${mv.to}`;
			case 'Retreat':
				return `Retreat pawn ${mv.pawn} → ${mv.to}`;
			case 'StartPawn':
				return `Start pawn ${mv.pawn} → ${mv.to}`;
			case 'Sorry':
				return `Sorry! pawn ${mv.my_pawn} bumps P${mv.their_player}(pawn ${mv.their_pawn}) → ${mv.to}`;
			case 'SwapEleven':
				return `Swap pawn ${mv.my_pawn} ↔ P${mv.their_player}(pawn ${mv.their_pawn})`;
			case 'SplitSeven':
				return `Split 7 → pawn ${mv.first.pawn} (${mv.first.steps}) + pawn ${mv.second.pawn} (${mv.second.steps})`;
			case 'Pass':
				return 'Pass';
		}
	});
</script>

<div class="probe">
	<header>
		<div class="lhs">
			<h1>Renderer probe</h1>
			{#if lastPlayedLabel}
				<span class="card-chip" style:color={lastPlayedLabel.color}>
					{lastPlayedLabel.label}
				</span>
			{/if}
			{#if lastMoveDesc}
				<span class="move-desc">{lastMoveDesc}</span>
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
			<div class="group" aria-label="Camera view">
				<button onclick={() => pickView('edge')} class:active={activePreset === 'edge'}>
					Edge
				</button>
				<button onclick={() => pickView('corner')} class:active={activePreset === 'corner'}>
					Corner
				</button>
				<button onclick={() => pickView('top')} class:active={activePreset === 'top'}>
					Top-down
				</button>
			</div>
			<div class="group" aria-label="Game">
				<button onclick={stepBot} disabled={stepping || gameOver || gameId === null}>
					Step bot
				</button>
				<button onclick={startGame}>New game</button>
			</div>
		</div>
	</header>
	<div class="canvas-wrap">
		{#if error}
			<p class="msg error">Error: {error}</p>
		{:else if !geometry || !gameState}
			<p class="msg">Loading board…</p>
		{:else}
			<BoardCanvas
				{geometry}
				skin={currentSkin}
				{gameState}
				{lastStep}
				{cameraCommand}
				{onUserOrbit}
				{onStepStart}
			/>
		{/if}
	</div>
	{#if geometry && gameState}
		<footer>
			<small>{turnLabel} · {geometry.spaces.length} spaces · {PIECE_MODEL_ATTRIBUTION}</small>
		</footer>
	{/if}
</div>

<style>
	.probe {
		display: flex;
		flex-direction: column;
		height: 100vh;
		background: #0e141c;
		color: #e8ecf2;
		font-family: system-ui, sans-serif;
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.75rem 1.25rem;
		border-bottom: 1px solid #253042;
		gap: 1rem;
		flex-wrap: wrap;
	}
	.lhs {
		display: flex;
		align-items: baseline;
		gap: 1rem;
	}
	h1 {
		margin: 0;
		font-size: 1rem;
		font-weight: 500;
	}
	.card-chip {
		font-weight: 700;
		font-size: 1.4rem;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0.02em;
		padding: 0.1rem 0.6rem;
		border-radius: 4px;
		background: #151b24;
	}
	.move-desc {
		font-size: 0.85rem;
		color: #8a96a8;
		font-family: ui-monospace, SFMono-Regular, monospace;
	}
	.controls {
		display: flex;
		gap: 1rem;
		flex-wrap: wrap;
	}
	.group {
		display: flex;
		gap: 0.4rem;
		background: #151b24;
		padding: 0.2rem;
		border-radius: 6px;
	}
	button {
		background: #253042;
		color: #e8ecf2;
		border: 0;
		padding: 0.4rem 0.9rem;
		cursor: pointer;
		border-radius: 4px;
		font: inherit;
	}
	button.active {
		background: #4f6b8a;
	}
	button:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.canvas-wrap {
		flex: 1;
		position: relative;
	}
	.msg {
		padding: 2rem;
		text-align: center;
	}
	.error {
		color: salmon;
	}
	footer {
		padding: 0.5rem 1.25rem;
		border-top: 1px solid #253042;
		font-size: 0.8rem;
		color: #8a96a8;
	}
</style>
