<script lang="ts">
	import { onMount } from 'svelte';

	import BoardCanvas, {
		type CameraCommand,
		type HighlightState,
		type StepCommand
	} from '$lib/board/BoardCanvas.svelte';
	import type { BoardGeometry, SpaceId } from '$lib/board/geometry';
	import { LIGHT_SKIN, DARK_SKIN } from '$lib/board/skins';
	import type { GameStateView } from '$lib/board/state';
	import type { CameraView } from '$lib/board/renderer';
	import type { PickHit } from '$lib/board/interaction';
	import {
		findLastPlay,
		type GameHistory,
		type PlayerAction,
		type ActionNeeded,
		type SplitLeg
	} from '$lib/board/actions';
	import {
		legalDestinationsForPawn,
		legalSecondLegDestinations,
		matchOpponentPickMove,
		matchSinglePickMove,
		matchSplitFirstLeg,
		matchSplitSecondLeg,
		onlyPassIsLegal,
		passIsLegal,
		pickablePawns,
		sortDestinationsByDistance
	} from '$lib/board/selection';
	import { PIECE_MODEL_ATTRIBUTION } from '$lib/board/assets';
	import { cardLabel, CARD_DISPLAY } from '$lib/board/cards';
	import { describeAction } from '$lib/board/a11y';
	import { loadWasm, parseJsonOrThrow } from '$lib/wasm';

	interface CreateResponse {
		game_id: number;
		state: GameStateView;
	}
	interface BotActionResponse {
		action: unknown;
		state: GameStateView;
	}
	interface StateResponse {
		state: GameStateView;
	}

	interface LastPlayed {
		player: number;
		card: string;
	}

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

	const PLAYER_NAMES = ['Red', 'Blue', 'Yellow', 'Green'];
	const PLACEMENT_ORDINAL = ['1st', '2nd', '3rd', '4th'];
	type Ruleset = 'Standard' | 'PlayOut';
	// `viewer` is the seat the human controls. `null` means observer mode
	// — all four players are bot-controlled. Defaults to P0 so a fresh
	// load is playable immediately.
	type ViewerSeat = 0 | 1 | 2 | 3 | null;

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
	let selectedPawn = $state<{ player: number; pawn: number } | null>(null);
	// While a Split-7 first leg is locked in, the UI waits for the user to
	// pick a second pawn + destination. Cancel button clears this.
	let pendingLeg1 = $state<SplitLeg | null>(null);
	// Keyboard-focused destination index — `Enter` commits the destination
	// at this index in `legalDestinations`. Mouse click still works and
	// bypasses this state. `null` = no focus (e.g. no pawn selected yet).
	let activeDestIdx = $state<number | null>(null);
	// ARIA-live announcement for the most recent completed move.
	let lastAnnouncement = $state<string>('');
	// When true, keep calling `apply_bot_action` after each animation
	// completes — pauses automatically on the viewer's turn and on game
	// over, so the user can step in manually.
	let autoStep = $state(false);
	// When true, automatically submit Pass on the viewer's turn *only*
	// if Pass is the single legal option. An 11 where Pass and Swap are
	// both legal stays interactive so the viewer can choose.
	let autoPass = $state(true);
	// When OFF (default), a click on an opponent who sits exactly 11
	// spaces ahead resolves to Advance-11 (bump). When ON, the same click
	// resolves to SwapEleven (trade). Affects both mouse and keyboard.
	// For opponents at any other distance only Swap is legal, so this
	// toggle has no effect there.
	let preferSwap11 = $state(false);
	let ruleset = $state<Ruleset>('Standard');
	let viewer = $state<ViewerSeat>(0);

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
						rules: ruleset
					})
				)
			);
			gameId = created.game_id;
			gameState = created.state;
			lastStep = undefined;
			lastPlayed = null;
			selectedPawn = null;
			pendingLeg1 = null;
			error = null;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function refreshAfterAction() {
		if (gameId === null) return;
		const wasm = await loadWasm();
		const post = parseJsonOrThrow<StateResponse>(wasm.get_game_state(gameId, -1)).state;
		const history = parseJsonOrThrow<GameHistory>(wasm.get_game_history(gameId));
		const play = findLastPlay(history, post.current_turn);
		gameState = post;
		if (play) {
			lastStep = {
				record: play.record,
				player: play.player,
				nonce: (lastStep?.nonce ?? 0) + 1
			};
		}
		selectedPawn = null;
		pendingLeg1 = null;
	}

	async function stepBot() {
		if (gameId === null || stepping || gameOver) return;
		// Belt-and-suspenders: `apply_bot_action` runs whichever strategy is
		// attached to the *current* player, which on the viewer's turn is
		// technically fine but semantically wrong — the bot would be
		// playing your hand. Refuse.
		const an = gameState?.action_needed;
		if (viewer !== null && an?.type === 'ChooseMove' && an.player === viewer) return;
		stepping = true;
		try {
			const wasm = await loadWasm();
			parseJsonOrThrow<BotActionResponse>(wasm.apply_bot_action(gameId, 'Random'));
			await refreshAfterAction();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			stepping = false;
		}
	}

	async function commitAction(action: PlayerAction) {
		if (gameId === null || stepping) return;
		stepping = true;
		try {
			const wasm = await loadWasm();
			parseJsonOrThrow<StateResponse>(wasm.apply_action(gameId, JSON.stringify(action)));
			await refreshAfterAction();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			stepping = false;
		}
	}

	function onPick(hit: PickHit | null) {
		if (!gameState || !viewerCanMove) return;
		const an = gameState.action_needed;
		if (an.type !== 'ChooseMove') return;

		if (hit === null) {
			selectedPawn = null;
			return;
		}

		// Any click that lands on (or next to) the viewer's Start platform
		// redirects to the canonical pickable Start pawn. Start pawns are
		// fungible: the engine only surfaces one `StartPawn` / `Sorry`
		// move per Start area, and all 4 pawn meshes there represent the
		// same interchangeable pieces. So whether the user hits the disk
		// edge, an empty wedge of the disk, or any of the 4 overlapping
		// pawn meshes, we always resolve to the pawn the engine is
		// willing to move.
		hit = redirectStartClick(hit, an, gameState, geometry);

		// Clicking one of our own pickable pawns always (re)selects it.
		if (hit.kind === 'pawn') {
			const ownPickable = pickablePawns(an.player, an.legal_moves).some(
				(p) => p.player === hit.player && p.pawn === hit.pawn
			);
			if (ownPickable) {
				// In Split-7 second-leg mode, the new pick must differ from
				// the locked first leg's pawn.
				if (pendingLeg1 && hit.pawn === pendingLeg1.pawn) return;
				selectedPawn = { player: hit.player, pawn: hit.pawn };
				return;
			}
		}

		// Anything below here requires a pawn already selected as the mover.
		if (!selectedPawn) return;

		// Resolve the click to a target space + (optional) target opponent.
		// Either mouse-clicking an opponent pawn directly, or keyboard /
		// space-clicking a ring on their tile, should yield the same
		// targetSpace + targetOpponent — so the bump-vs-swap precedence
		// below applies uniformly whichever input the user used.
		const positions = gameState.pawn_positions;
		let targetSpace: SpaceId | null = null;
		let targetOpponent: { player: number; pawn: number } | null = null;
		if (hit.kind === 'space') {
			targetSpace = hit.spaceId;
			for (let p = 0; p < gameState.num_players; p++) {
				if (p === an.player) continue;
				const idx = positions[p]?.indexOf(targetSpace);
				if (idx !== undefined && idx >= 0) {
					targetOpponent = { player: p, pawn: idx };
					break;
				}
			}
		} else {
			targetOpponent = { player: hit.player, pawn: hit.pawn };
			const space = positions[hit.player]?.[hit.pawn];
			if (space !== undefined) targetSpace = space;
		}

		// Split-7 second leg flow.
		if (pendingLeg1) {
			if (targetSpace == null) return;
			const action = matchSplitSecondLeg(
				an.legal_moves,
				pendingLeg1,
				selectedPawn.pawn,
				targetSpace
			);
			if (action) void commitAction(action);
			return;
		}

		// Compute both possible matches — single-space (Advance/Retreat/
		// StartPawn/Sorry-by-space) and opponent-pair (Sorry/SwapEleven).
		// Pick between them using `preferSwap11` only when they represent
		// a genuine choice (different move types — the 11-at-11 dilemma).
		const spaceMatch =
			targetSpace != null
				? matchSinglePickMove(an.legal_moves, selectedPawn.pawn, targetSpace)
				: null;
		const swapMatch = targetOpponent
			? matchOpponentPickMove(
					an.legal_moves,
					selectedPawn.pawn,
					targetOpponent.player,
					targetOpponent.pawn
				)
			: null;
		let chosen: PlayerAction | null;
		if (
			spaceMatch &&
			swapMatch &&
			spaceMatch.type === 'PlayMove' &&
			swapMatch.type === 'PlayMove' &&
			spaceMatch.mv.type !== swapMatch.mv.type
		) {
			chosen = preferSwap11 ? swapMatch : spaceMatch;
		} else {
			chosen = spaceMatch ?? swapMatch;
		}
		if (chosen) {
			void commitAction(chosen);
			return;
		}

		// Split-7 first leg falls through here (neither single-pick nor
		// opponent-pick matched).
		if (targetSpace != null) {
			const firstLeg = matchSplitFirstLeg(
				an.legal_moves,
				selectedPawn.pawn,
				targetSpace
			);
			if (firstLeg) {
				pendingLeg1 = firstLeg;
				selectedPawn = null;
			}
		}
	}

	function cancelSplit() {
		pendingLeg1 = null;
		selectedPawn = null;
	}

	function setRuleset(r: Ruleset) {
		if (r === ruleset) return;
		ruleset = r;
		// Ruleset change resets the game — it affects engine construction.
		void startGame();
	}

	function setViewer(seat: ViewerSeat) {
		if (seat === viewer) return;
		viewer = seat;
		// Any in-flight selection belongs to the previous seat; drop it.
		selectedPawn = null;
		pendingLeg1 = null;
		activeDestIdx = null;
		// Rotate the camera to the new seat's edge view. Observer keeps
		// whatever view they had.
		if (seat !== null) {
			cameraCommand = {
				view: 'edge',
				nonce: (cameraCommand?.nonce ?? 0) + 1,
				targetAzimuth: edgeAzimuthForPlayer(seat)
			};
			activePreset = 'edge';
		}
	}

	// Each seat's "edge view" puts that seat's side of the board between
	// the viewer and the center — i.e. the seat's colored start disk sits
	// at the bottom of the screen. Azimuth is measured around the board's
	// Y axis, with 0 pointing at P2 (default camera position).
	function edgeAzimuthForPlayer(seat: Exclude<ViewerSeat, null>): number {
		// Geometry sides:
		//   P0 Red    — side 0, z = -1.8 world  →  camera belongs at -z (azimuth π)
		//   P1 Blue   — side 1, x = +1.8 world  →  camera at +x (azimuth π/2)
		//   P2 Yellow — side 2, z = +1.8 world  →  camera at +z (azimuth 0)
		//   P3 Green  — side 3, x = -1.8 world  →  camera at -x (azimuth -π/2)
		const map = [Math.PI, Math.PI / 2, 0, -Math.PI / 2];
		return map[seat] ?? 0;
	}

	function redirectStartClick(
		hit: PickHit,
		an: Extract<ActionNeeded, { type: 'ChooseMove' }>,
		gs: GameStateView,
		geom: BoardGeometry | null
	): PickHit {
		if (!geom) return hit;
		let startSpace: SpaceId | null = null;
		if (hit.kind === 'space') {
			const spaceId = hit.spaceId;
			const layout = geom.spaces.find((s) => s.id === spaceId);
			if (layout && 'StartArea' in layout.kind && layout.kind.StartArea === an.player) {
				startSpace = spaceId;
			}
		} else if (hit.player === an.player) {
			const pawnPos = gs.pawn_positions[hit.player]?.[hit.pawn];
			if (pawnPos !== undefined) {
				const layout = geom.spaces.find((s) => s.id === pawnPos);
				if (layout && 'StartArea' in layout.kind && layout.kind.StartArea === an.player) {
					startSpace = pawnPos;
				}
			}
		}
		if (startSpace === null) return hit;
		const candidate = pickablePawns(an.player, an.legal_moves).find(
			(p) => p.player === an.player && gs.pawn_positions[p.player]?.[p.pawn] === startSpace
		);
		if (!candidate) return hit;
		return { kind: 'pawn', player: candidate.player, pawn: candidate.pawn };
	}

	function onKeyDown(e: KeyboardEvent) {
		if (!gameState || !viewerCanMove) return;
		const an = gameState.action_needed;
		if (an.type !== 'ChooseMove') return;

		// Don't intercept keystrokes that are going to a focusable control
		// (e.g. the user tabbed into a header button and pressed Enter).
		const target = e.target as Element | null;
		if (target && (target.tagName === 'BUTTON' || target.tagName === 'INPUT')) {
			return;
		}

		if (e.key === 'Escape') {
			e.preventDefault();
			if (pendingLeg1) cancelSplit();
			else selectedPawn = null;
			return;
		}

		if (e.key === 'Tab') {
			const pawns = pickablePawns(an.player, an.legal_moves);
			if (pawns.length === 0) return;
			// Split-7 second leg — the pending first-leg's pawn isn't
			// pickable; cycle among the remaining.
			const cyclable = pendingLeg1
				? pawns.filter((p) => p.pawn !== pendingLeg1!.pawn)
				: pawns;
			if (cyclable.length === 0) return;
			e.preventDefault();
			const currentIdx = selectedPawn
				? cyclable.findIndex(
						(p) => p.player === selectedPawn!.player && p.pawn === selectedPawn!.pawn
					)
				: -1;
			const delta = e.shiftKey ? -1 : 1;
			const nextIdx = (currentIdx + delta + cyclable.length) % cyclable.length;
			selectedPawn = cyclable[nextIdx];
			activeDestIdx = null; // reset; effect below will set to 0 if any dests
			return;
		}

		if (
			e.key === 'ArrowLeft' ||
			e.key === 'ArrowRight' ||
			e.key === 'ArrowUp' ||
			e.key === 'ArrowDown'
		) {
			if (legalDestinations.length === 0) return;
			e.preventDefault();
			const delta = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : -1;
			const base = activeDestIdx ?? -1;
			const next = (base + delta + legalDestinations.length) % legalDestinations.length;
			activeDestIdx = next;
			return;
		}

		if (e.key === 'Enter' || e.key === ' ') {
			// If the user pressed Enter without selecting a pawn yet, cycle to
			// the first pickable as an onboarding shortcut.
			if (!selectedPawn) {
				const pawns = pickablePawns(an.player, an.legal_moves);
				if (pawns.length > 0) {
					e.preventDefault();
					selectedPawn = pawns[0];
				}
				return;
			}
			if (legalDestinations.length === 0) return;
			e.preventDefault();
			const idx = activeDestIdx ?? 0;
			const space = legalDestinations[idx];
			onPick({ kind: 'space', spaceId: space });
			return;
		}
	}

	// Whenever the destination list changes (pawn change, state change,
	// leg swap), snap the keyboard focus to the first destination so
	// Enter always has something to commit.
	$effect(() => {
		void legalDestinations; // track dependency
		if (legalDestinations.length === 0) {
			activeDestIdx = null;
		} else if (activeDestIdx === null || activeDestIdx >= legalDestinations.length) {
			activeDestIdx = 0;
		}
	});

	async function passMove() {
		const an = gameState?.action_needed;
		if (an?.type !== 'ChooseMove') return;
		// Only ever pass for the viewer. Bots route through `stepBot`
		// (which runs the engine's strategy), not this button.
		if (viewer === null || an.player !== viewer) return;
		const pass = an.legal_moves.find((m) => m.type === 'Pass');
		if (!pass) return;
		await commitAction({ type: 'PlayMove', mv: pass });
	}

	function pickView(view: CameraView) {
		activePreset = view;
		cameraCommand = { view, nonce: (cameraCommand?.nonce ?? 0) + 1 };
	}

	function onUserOrbit() {
		activePreset = null;
	}

	function onStepStart(step: StepCommand) {
		lastPlayed = { player: step.player, card: step.record.card };
	}

	function onHover(hit: PickHit | null) {
		if (!viewerCanMove || hit === null) return;
		// Resolve the hover target to a destination space — either the
		// space directly, or the current tile of a hovered opponent pawn
		// (which is also where a click would commit a swap/bump).
		let hoveredSpace: SpaceId | null = null;
		if (hit.kind === 'space') {
			hoveredSpace = hit.spaceId;
		} else if (gameState && hit.player !== viewer) {
			const space = gameState.pawn_positions[hit.player]?.[hit.pawn];
			if (space !== undefined) hoveredSpace = space;
		}
		if (hoveredSpace === null) return;
		const idx = legalDestinations.indexOf(hoveredSpace);
		if (idx >= 0) activeDestIdx = idx;
	}

	function onStepEnd(step: StepCommand) {
		if (geometry) {
			lastAnnouncement = describeAction(step.record, step.player, geometry);
		}
		// Chain the next bot step only *after* the current step's animation
		// finishes, so we don't blast through the queue.
		if (canAutoStep()) void stepBot();
	}

	function canAutoStep(): boolean {
		if (!autoStep) return false;
		if (stepping || gameOver) return false;
		const s = gameState;
		if (!s) return false;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove') return false;
		// Observer mode (viewer === null) bot-plays every turn.
		return viewer === null || an.player !== viewer;
	}

	// Kick off the chain the moment the user toggles auto-step ON and the
	// current turn belongs to a bot. Subsequent steps chain via `onStepEnd`.
	$effect(() => {
		void autoStep;
		if (canAutoStep()) void stepBot();
	});

	function shouldAutoPass(): boolean {
		if (!autoPass || stepping || gameOver) return false;
		const s = gameState;
		if (!s) return false;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove') return false;
		if (viewer === null || an.player !== viewer) return false;
		return onlyPassIsLegal(an.legal_moves);
	}

	// Auto-pass with a short dwell so the "Your turn (N)" banner has time
	// to appear — reassures the viewer they saw their own unplayable draw
	// before the turn evaporates. The cleanup cancels the timer if state
	// changes (e.g. the user toggles auto-pass off) before it fires.
	$effect(() => {
		if (!shouldAutoPass()) return;
		const timer = setTimeout(() => {
			// Re-check just before firing. If any legal move other than
			// Pass has appeared (state mutated during the dwell), abort —
			// the viewer should have a chance to pick it.
			if (!shouldAutoPass()) return;
			const an = gameState?.action_needed;
			if (an?.type !== 'ChooseMove') return;
			const nonPassCount = an.legal_moves.filter((m) => m.type !== 'Pass').length;
			if (nonPassCount > 0) {
				console.warn(
					'[auto-pass] skipping — non-Pass moves exist',
					an.legal_moves
				);
				return;
			}
			void passMove();
		}, 500);
		return () => clearTimeout(timer);
	});

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
		if (s.winners.length > 0)
			return `Game over — ${PLAYER_NAMES[s.winners[0]] ?? `P${s.winners[0]}`} wins`;
		if (s.truncated) return 'Game over (truncated)';
		return `Turn ${s.turn_count + 1} · ${PLAYER_NAMES[s.current_player] ?? `P${s.current_player}`}`;
	});
	const lastPlayedLabel = $derived.by(() => {
		const lp = lastPlayed;
		if (!lp) return null;
		return {
			label: cardLabel(lp.card) ?? lp.card,
			color: currentSkin.palette.players[lp.player]
		};
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
	const viewerCanMove = $derived.by(() => {
		const s = gameState;
		if (!s || stepping || gameOver || viewer === null) return false;
		const an = s.action_needed;
		return an.type === 'ChooseMove' && an.player === viewer;
	});
	const legalDestinations = $derived.by<SpaceId[]>(() => {
		if (!viewerCanMove || !gameState) return [];
		const an = gameState.action_needed as Extract<ActionNeeded, { type: 'ChooseMove' }>;
		// Second-leg phase of a Split-7: highlight where the newly-selected
		// pawn can land given the locked first leg.
		let raw: SpaceId[];
		let referencePawn: { player: number; pawn: number } | null = null;
		if (pendingLeg1) {
			if (!selectedPawn) return [];
			raw = legalSecondLegDestinations(an.legal_moves, pendingLeg1, selectedPawn.pawn);
			referencePawn = selectedPawn;
		} else {
			if (!selectedPawn) return [];
			raw = legalDestinationsForPawn(
				an.legal_moves,
				selectedPawn.pawn,
				gameState.pawn_positions
			);
			referencePawn = selectedPawn;
		}
		if (geometry && referencePawn) {
			const myPawnSpace =
				gameState.pawn_positions[referencePawn.player]?.[referencePawn.pawn];
			if (myPawnSpace !== undefined) {
				return sortDestinationsByDistance(geometry, referencePawn.player, myPawnSpace, raw);
			}
		}
		return raw;
	});
	const highlights = $derived.by<HighlightState>(() => ({
		destinations: legalDestinations,
		activeDestination:
			activeDestIdx != null && activeDestIdx < legalDestinations.length
				? legalDestinations[activeDestIdx]
				: null,
		selectedPawn,
		lockedPawn:
			pendingLeg1 && viewer !== null ? { player: viewer, pawn: pendingLeg1.pawn } : null,
		lockedDestination: pendingLeg1 ? pendingLeg1.to : null,
		currentPlayer: gameState?.current_player ?? 0
	}));
	const canPass = $derived.by(() => {
		if (!viewerCanMove || !gameState) return false;
		const an = gameState.action_needed as Extract<ActionNeeded, { type: 'ChooseMove' }>;
		return passIsLegal(an.legal_moves);
	});
</script>

<svelte:window onkeydown={onKeyDown} />

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
			{#if viewerCanMove && viewer !== null}
				<span class="you-up" style:color={currentSkin.palette.players[viewer]}>
					Your turn ({cardLabel(gameState?.drawn_card ?? null) ?? '—'})
				</span>
			{/if}
			{#if pendingLeg1}
				<span class="split-status">
					Split 7: leg 1 = pawn {pendingLeg1.pawn} → {pendingLeg1.to} ({pendingLeg1.steps}
					steps). Pick a different pawn + destination for leg 2.
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
			<div class="group" aria-label="Ruleset">
				<button
					onclick={() => setRuleset('Standard')}
					class:active={ruleset === 'Standard'}
					title="Standard Sorry! — first player home wins."
				>
					Standard
				</button>
				<button
					onclick={() => setRuleset('PlayOut')}
					class:active={ruleset === 'PlayOut'}
					title="Continue play after the winner is decided to establish 1st→4th placement order."
				>
					Play Out
				</button>
			</div>
			<div class="group" aria-label="Your seat">
				{#each [0, 1, 2, 3] as seat (seat)}
					<button
						onclick={() => setViewer(seat as ViewerSeat)}
						class:active={viewer === seat}
						style:color={viewer === seat ? currentSkin.palette.players[seat] : undefined}
						title="Play as {PLAYER_NAMES[seat]} (P{seat})"
					>
						{PLAYER_NAMES[seat]}
					</button>
				{/each}
				<button
					onclick={() => setViewer(null)}
					class:active={viewer === null}
					title="Observer — all players bot-controlled"
				>
					Observer
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
				{#if pendingLeg1}
					<button onclick={cancelSplit} disabled={stepping}>Cancel split</button>
				{/if}
				{#if canPass}
					<button onclick={passMove} disabled={stepping}>Pass</button>
				{/if}
				<button
					onclick={stepBot}
					disabled={stepping || gameOver || gameId === null || viewerCanMove}
				>
					Step bot
				</button>
				<button
					onclick={() => (autoStep = !autoStep)}
					class:active={autoStep}
					disabled={gameOver || gameId === null}
					aria-pressed={autoStep}
				>
					{autoStep ? 'Auto-step: on' : 'Auto-step: off'}
				</button>
				<button
					onclick={() => (autoPass = !autoPass)}
					class:active={autoPass}
					disabled={gameOver || gameId === null}
					aria-pressed={autoPass}
					title="Automatically pass on your turn when Pass is the only legal option"
				>
					{autoPass ? 'Auto-pass: on' : 'Auto-pass: off'}
				</button>
				<button
					onclick={() => (preferSwap11 = !preferSwap11)}
					class:active={preferSwap11}
					disabled={gameOver || gameId === null}
					aria-pressed={preferSwap11}
					title="On an 11, clicking an opponent exactly 11 spaces away resolves to either Bump (default) or Swap"
				>
					11 @ 11: {preferSwap11 ? 'Swap' : 'Bump'}
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
				{highlights}
				{onUserOrbit}
				{onStepStart}
				{onStepEnd}
				{onPick}
				{onHover}
			/>
		{/if}
		{#if gameState && gameOver && gameState.winners.length > 0}
			<div
				class="placement-banner"
				role="status"
				aria-live="polite"
				aria-label="Final placements"
			>
				<h2>Game over</h2>
				<ol>
					{#each gameState.winners as player, i (player)}
						<li style:color={currentSkin.palette.players[player] ?? '#e8ecf2'}>
							<span class="ordinal">{PLACEMENT_ORDINAL[i] ?? `${i + 1}th`}</span>
							<span class="name">{PLAYER_NAMES[player] ?? `P${player}`}</span>
						</li>
					{/each}
				</ol>
			</div>
		{/if}
	</div>
	{#if geometry && gameState}
		<footer>
			<small>{turnLabel} · {geometry.spaces.length} spaces · {PIECE_MODEL_ATTRIBUTION}</small>
		</footer>
	{/if}

	<!-- ARIA live region for screen-reader move announcements. -->
	<div class="visually-hidden" aria-live="polite" aria-atomic="true">
		{lastAnnouncement}
	</div>
</div>

<style>
	/* Lock the page to the viewport so the header + canvas + footer
	   together occupy exactly the visible area, no scroll. The probe owns
	   this globally since it's the primary full-viewport surface. */
	:global(html),
	:global(body) {
		margin: 0;
		padding: 0;
		height: 100%;
		overflow: hidden;
	}
	.probe {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		background: #0e141c;
		color: #e8ecf2;
		font-family: system-ui, sans-serif;
		overflow: hidden;
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 1.25rem;
		border-bottom: 1px solid #253042;
		gap: 0.75rem;
		flex-wrap: wrap;
		flex: 0 0 auto;
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
	.you-up {
		font-weight: 600;
		font-size: 0.9rem;
	}
	.split-status {
		font-size: 0.85rem;
		color: #f2db88;
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
		flex: 1 1 0;
		/* Required so a flex child with `flex: 1` actually shrinks below
		   the natural height of the <canvas> inside it — otherwise the
		   canvas's default intrinsic size prevents the wrap from fitting
		   and overflows the viewport. */
		min-height: 0;
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
		padding: 0.4rem 1.25rem;
		border-top: 1px solid #253042;
		font-size: 0.8rem;
		color: #8a96a8;
		flex: 0 0 auto;
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
	.placement-banner {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		background: rgba(20, 28, 40, 0.92);
		backdrop-filter: blur(8px);
		border: 1px solid #4f6b8a;
		border-radius: 12px;
		padding: 1.5rem 2rem;
		min-width: 16rem;
		box-shadow: 0 12px 48px rgba(0, 0, 0, 0.5);
		text-align: center;
		pointer-events: none;
	}
	.placement-banner h2 {
		margin: 0 0 1rem;
		font-size: 1.4rem;
		font-weight: 600;
		color: #e8ecf2;
	}
	.placement-banner ol {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}
	.placement-banner li {
		display: flex;
		justify-content: space-between;
		gap: 1.25rem;
		font-size: 1.05rem;
		font-weight: 600;
	}
	.placement-banner .ordinal {
		color: #8a96a8;
		font-weight: 500;
		font-variant-numeric: tabular-nums;
	}
	.placement-banner .name {
		font-weight: 700;
	}
</style>
