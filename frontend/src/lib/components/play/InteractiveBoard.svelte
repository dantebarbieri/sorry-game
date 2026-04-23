<script lang="ts">
	import BoardCanvas, {
		type CameraCommand,
		type HighlightState,
		type StepCommand
	} from '$lib/board/BoardCanvas.svelte';
	import type { BoardGeometry, SpaceId } from '$lib/board/geometry';
	import type { BoardSkin } from '$lib/board/skins';
	import type { GameStateView } from '$lib/board/state';
	import type { CameraView } from '$lib/board/renderer';
	import type { PickHit } from '$lib/board/interaction';
	import type {
		PlayerAction,
		ActionNeeded,
		SplitLeg
	} from '$lib/board/actions';
	import {
		legalDestinationsForPawn,
		legalSecondLegDestinations,
		matchOpponentPickMove,
		matchSinglePickMove,
		matchSplitFirstLeg,
		matchSplitSecondLeg,
		onlyPassIsLegal,
		pickablePawns,
		sortDestinationsByDistance
	} from '$lib/board/selection';
	import { describeAction } from '$lib/board/a11y';
	import type { PlayController, ViewerSeat } from '$lib/play/types';

	interface Props {
		controller: PlayController;
		viewer: ViewerSeat;
		skin: BoardSkin;
		autoStep: boolean;
		autoPass: boolean;
		preferSwap11: boolean;
		cameraCommand: CameraCommand | undefined;
		onCameraOrbit: () => void;
		onAnnouncement: (text: string) => void;
		/** Bindable — parent HUD reads pendingLeg1 to render the split status. */
		pendingLeg1?: SplitLeg | null;
		/** Bindable — parent uses this to show/hide Pass and Cancel-split. */
		selectedPawn?: { player: number; pawn: number } | null;
	}

	let {
		controller,
		viewer,
		skin,
		autoStep,
		autoPass,
		preferSwap11,
		cameraCommand,
		onCameraOrbit,
		onAnnouncement,
		pendingLeg1 = $bindable(null),
		selectedPawn = $bindable(null)
	}: Props = $props();

	let activeDestIdx = $state<number | null>(null);
	let lastStep = $derived(controller.lastStep);

	// Whenever a play finalizes (new `lastStep`), clear any in-flight
	// selection. Without this, state from the previous turn — especially
	// a `pendingLeg1` from a Split-7 — bleeds into the next turn and makes
	// the UI appear locked. The probe used to do this inside its
	// `refreshAfterAction`; the controller split means we do it here.
	let lastSeenStepNonce = $state<number | null>(null);
	$effect(() => {
		const n = lastStep?.nonce ?? null;
		if (n !== lastSeenStepNonce) {
			lastSeenStepNonce = n;
			selectedPawn = null;
			pendingLeg1 = null;
			activeDestIdx = null;
		}
	});

	const gameOver = $derived(controller.gameOver);

	const viewerCanMove = $derived.by(() => {
		const s = controller.gameState;
		if (!s || controller.stepping || gameOver || viewer === null) return false;
		const an = s.action_needed;
		return an.type === 'ChooseMove' && an.player === viewer;
	});

	const legalDestinations = $derived.by<SpaceId[]>(() => {
		if (!viewerCanMove || !controller.gameState) return [];
		const an = controller.gameState.action_needed as Extract<ActionNeeded, { type: 'ChooseMove' }>;
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
				controller.gameState.pawn_positions
			);
			referencePawn = selectedPawn;
		}
		const geom = controller.geometry;
		if (geom && referencePawn) {
			const myPawnSpace =
				controller.gameState.pawn_positions[referencePawn.player]?.[referencePawn.pawn];
			if (myPawnSpace !== undefined) {
				return sortDestinationsByDistance(geom, referencePawn.player, myPawnSpace, raw);
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
		currentPlayer: controller.gameState?.current_player ?? 0
	}));

	function cancelSplit() {
		pendingLeg1 = null;
		selectedPawn = null;
	}

	async function passMove() {
		if (viewer === null) return;
		await controller.passFor(viewer);
	}

	function onPick(hit: PickHit | null) {
		const s = controller.gameState;
		if (!s || !viewerCanMove) return;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove') return;

		if (hit === null) {
			selectedPawn = null;
			return;
		}

		hit = redirectStartClick(hit, an, s, controller.geometry);

		if (hit.kind === 'pawn') {
			const ownPickable = pickablePawns(an.player, an.legal_moves).some(
				(p) => p.player === hit.player && p.pawn === hit.pawn
			);
			if (ownPickable) {
				if (pendingLeg1 && hit.pawn === pendingLeg1.pawn) return;
				selectedPawn = { player: hit.player, pawn: hit.pawn };
				return;
			}
		}

		if (!selectedPawn) return;

		const positions = s.pawn_positions;
		let targetSpace: SpaceId | null = null;
		let targetOpponent: { player: number; pawn: number } | null = null;
		if (hit.kind === 'space') {
			targetSpace = hit.spaceId;
			for (let p = 0; p < s.num_players; p++) {
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

		if (pendingLeg1) {
			if (targetSpace == null) return;
			const action = matchSplitSecondLeg(
				an.legal_moves,
				pendingLeg1,
				selectedPawn.pawn,
				targetSpace
			);
			if (action) void controller.commitAction(action);
			return;
		}

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
			void controller.commitAction(chosen);
			return;
		}

		if (targetSpace != null) {
			const firstLeg = matchSplitFirstLeg(an.legal_moves, selectedPawn.pawn, targetSpace);
			if (firstLeg) {
				pendingLeg1 = firstLeg;
				selectedPawn = null;
			}
		}
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

	function onHover(hit: PickHit | null) {
		if (!viewerCanMove || hit === null) return;
		const s = controller.gameState;
		if (!s) return;
		let hoveredSpace: SpaceId | null = null;
		if (hit.kind === 'space') {
			hoveredSpace = hit.spaceId;
		} else if (hit.player !== viewer) {
			const space = s.pawn_positions[hit.player]?.[hit.pawn];
			if (space !== undefined) hoveredSpace = space;
		}
		if (hoveredSpace === null) return;
		const idx = legalDestinations.indexOf(hoveredSpace);
		if (idx >= 0) activeDestIdx = idx;
	}

	function onStepEnd(step: StepCommand) {
		const geom = controller.geometry;
		if (geom) {
			onAnnouncement(describeAction(step.record, step.player, geom));
		}
		if (canAutoStep()) void controller.stepBot();
	}

	function canAutoStep(): boolean {
		if (!autoStep) return false;
		if (controller.stepping || gameOver) return false;
		const s = controller.gameState;
		if (!s) return false;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove' && an.type !== 'ChooseCard') return false;
		return viewer === null || an.player !== viewer;
	}

	$effect(() => {
		void autoStep;
		void controller.gameState;
		if (canAutoStep()) void controller.stepBot();
	});

	$effect(() => {
		void legalDestinations;
		if (legalDestinations.length === 0) {
			activeDestIdx = null;
		} else if (activeDestIdx === null || activeDestIdx >= legalDestinations.length) {
			activeDestIdx = 0;
		}
	});

	function shouldAutoPass(): boolean {
		if (!autoPass || controller.stepping || gameOver) return false;
		const s = controller.gameState;
		if (!s) return false;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove') return false;
		if (viewer === null || an.player !== viewer) return false;
		return onlyPassIsLegal(an.legal_moves);
	}

	$effect(() => {
		if (!shouldAutoPass()) return;
		const timer = setTimeout(() => {
			if (!shouldAutoPass()) return;
			const an = controller.gameState?.action_needed;
			if (an?.type !== 'ChooseMove') return;
			const nonPassCount = an.legal_moves.filter((m) => m.type !== 'Pass').length;
			if (nonPassCount > 0) return;
			void passMove();
		}, 500);
		return () => clearTimeout(timer);
	});

	function onKeyDown(e: KeyboardEvent) {
		const s = controller.gameState;
		if (!s || !viewerCanMove) return;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove') return;

		const target = e.target as Element | null;
		if (target && (target.tagName === 'BUTTON' || target.tagName === 'INPUT' || target.tagName === 'SELECT')) return;

		if (e.key === 'Escape') {
			e.preventDefault();
			if (pendingLeg1) cancelSplit();
			else selectedPawn = null;
			return;
		}
		if (e.key === 'Tab') {
			const pawns = pickablePawns(an.player, an.legal_moves);
			if (pawns.length === 0) return;
			const cyclable = pendingLeg1 ? pawns.filter((p) => p.pawn !== pendingLeg1!.pawn) : pawns;
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
			activeDestIdx = null;
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
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="canvas-wrap">
	{#if controller.error}
		<p class="msg error">Error: {controller.error}</p>
	{:else if !controller.geometry || !controller.gameState}
		<p class="msg">Loading board…</p>
	{:else}
		<BoardCanvas
			geometry={controller.geometry}
			{skin}
			gameState={controller.gameState}
			{lastStep}
			{cameraCommand}
			{highlights}
			onUserOrbit={onCameraOrbit}
			{onStepEnd}
			{onPick}
			{onHover}
		/>
	{/if}
</div>

<style>
	.canvas-wrap {
		flex: 1 1 0;
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
</style>
