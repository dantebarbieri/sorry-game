<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { CameraCommand } from '$lib/board/BoardCanvas.svelte';
	import type { CameraView } from '$lib/board/renderer';
	import { passIsLegal } from '$lib/board/selection';
	import type { ActionNeeded, SplitLeg } from '$lib/board/actions';
	import { LocalController } from '$lib/play/local-controller.svelte';
	import { OnlineController } from '$lib/play/online-controller.svelte';
	import {
		activeSeatSides,
		type PlayController,
		type ViewerSeat
	} from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';
	import LocalPreGame from '$lib/components/play/LocalPreGame.svelte';
	import Hud from '$lib/components/play/Hud.svelte';
	import PlayControls from '$lib/components/play/PlayControls.svelte';
	import InteractiveBoard from '$lib/components/play/InteractiveBoard.svelte';
	import OnlineLobby from '$lib/components/play/OnlineLobby.svelte';

	const theme = useTheme();
	const local = new LocalController();
	const online = new OnlineController();

	let mode = $state<'local' | 'online'>('local');
	let viewer = $state<ViewerSeat>(0);
	let autoStep = $state(true);
	let autoPass = $state(true);
	let preferSwap11 = $state(false);
	let activePreset = $state<CameraView | null>('edge');
	let cameraCommand = $state<CameraCommand | undefined>(undefined);
	let lastAnnouncement = $state('');
	let selectedPawn = $state<{ player: number; pawn: number } | null>(null);
	let pendingLeg1 = $state<SplitLeg | null>(null);
	let localNewGameRequested = $state(false);

	onDestroy(() => {
		local.destroy();
		online.disconnect();
	});

	function edgeAzimuthForSide(side: number): number {
		const map = [Math.PI, Math.PI / 2, 0, -Math.PI / 2];
		return map[side] ?? 0;
	}

	/** Rotate the camera to the given engine player's board side. */
	function rotateCameraTo(engineIdx: ViewerSeat) {
		if (engineIdx === null) return;
		const sides = activeSeatSides(activeController.setup);
		const side = sides[engineIdx] ?? engineIdx;
		cameraCommand = {
			view: 'edge',
			nonce: (cameraCommand?.nonce ?? 0) + 1,
			targetAzimuth: edgeAzimuthForSide(side)
		};
		activePreset = 'edge';
	}

	const activeController = $derived<PlayController>(mode === 'local' ? local : online);

	// Hotseat viewer rotation — only relevant locally. Online, the server
	// decides whose turn it is and the viewer is fixed to the client's seat.
	$effect(() => {
		if (mode !== 'local') return;
		const s = local.gameState;
		const setup = local.setup;
		if (!s || s.winners.length > 0 || s.truncated) return;
		const sides = activeSeatSides(setup);
		const humanEngineIdxs = sides
			.map((side, engineIdx) => (setup.seats[side].type === 'Human' ? engineIdx : -1))
			.filter((i) => i >= 0);
		if (humanEngineIdxs.length <= 1) return;
		const an = s.action_needed;
		if (an.type !== 'ChooseMove' && an.type !== 'ChooseCard') return;
		if (humanEngineIdxs.includes(an.player) && an.player !== viewer) {
			viewer = an.player;
			rotateCameraTo(viewer);
			selectedPawn = null;
			pendingLeg1 = null;
		}
	});

	// Online: viewer is fixed to the player slot assigned by the server.
	$effect(() => {
		if (mode !== 'online') return;
		if (online.viewer !== null && online.viewer !== viewer) {
			viewer = online.viewer;
			rotateCameraTo(viewer);
			selectedPawn = null;
			pendingLeg1 = null;
		}
	});

	function onLocalGameStarted(v: ViewerSeat) {
		viewer = v;
		rotateCameraTo(viewer);
		selectedPawn = null;
		pendingLeg1 = null;
	}

	function onPickView(view: CameraView) {
		activePreset = view;
		cameraCommand = { view, nonce: (cameraCommand?.nonce ?? 0) + 1 };
	}

	function onCameraOrbit() {
		activePreset = null;
	}

	const viewerCanMove = $derived.by(() => {
		const s = activeController.gameState;
		if (!s || activeController.stepping || activeController.gameOver || viewer === null) return false;
		const an = s.action_needed;
		return an.type === 'ChooseMove' && an.player === viewer;
	});

	const canPass = $derived.by(() => {
		const s = activeController.gameState;
		if (!viewerCanMove || !s) return false;
		const an = s.action_needed as Extract<ActionNeeded, { type: 'ChooseMove' }>;
		return passIsLegal(an.legal_moves);
	});

	const canStepBot = $derived.by(() => {
		if (mode === 'online') return false;
		if (local.stepping || local.gameOver || !local.gameState) return false;
		return !viewerCanMove;
	});

	async function onPassClick() {
		if (viewer === null) return;
		await activeController.passFor(viewer);
	}

	function onCancelSplit() {
		pendingLeg1 = null;
		selectedPawn = null;
	}

	function onNewGame() {
		if (mode === 'online') {
			void online.newGame();
		} else {
			localNewGameRequested = true;
		}
	}

	const showBoard = $derived.by(() => {
		if (mode === 'online') return !!(online.lobby && online.lobby.phase !== 'lobby');
		return local.gameState !== null;
	});
</script>

<div class="play">
	<div class="tabs">
		<button
			onclick={() => (mode = 'local')}
			class:active={mode === 'local'}
			aria-pressed={mode === 'local'}
		>
			Local
		</button>
		<button
			onclick={() => (mode = 'online')}
			class:active={mode === 'online'}
			aria-pressed={mode === 'online'}
		>
			Online
		</button>
	</div>

	{#if mode === 'online' && !showBoard}
		<OnlineLobby controller={online} onConnected={() => { /* lobby shown until phase leaves 'lobby' */ }} />
	{:else if !showBoard}
		<!-- Local mode pre-game pane; the drawer lives in LocalPreGame and
		     also handles the mid-game "new game" reopen. -->
	{:else}
		<Hud
			skin={theme.skin}
			gameState={activeController.gameState}
			lastStep={activeController.lastStep}
			{viewer}
			stepping={activeController.stepping}
			{viewerCanMove}
			{pendingLeg1}
			{lastAnnouncement}
		/>
		<PlayControls
			{canPass}
			canCancelSplit={pendingLeg1 !== null}
			stepping={activeController.stepping}
			gameOver={activeController.gameOver}
			{canStepBot}
			{autoStep}
			{autoPass}
			{preferSwap11}
			{activePreset}
			onPass={onPassClick}
			onCancelSplit={onCancelSplit}
			onStepBot={() => void local.stepBot()}
			onToggleAutoStep={() => (autoStep = !autoStep)}
			onToggleAutoPass={() => (autoPass = !autoPass)}
			onTogglePreferSwap11={() => (preferSwap11 = !preferSwap11)}
			onNewGame={onNewGame}
			{onPickView}
		/>
		<InteractiveBoard
			controller={activeController}
			{viewer}
			skin={theme.skin}
			{autoStep}
			{autoPass}
			{preferSwap11}
			{cameraCommand}
			{onCameraOrbit}
			onAnnouncement={(text) => (lastAnnouncement = text)}
			bind:pendingLeg1
			bind:selectedPawn
		/>
	{/if}
</div>

{#if mode === 'local'}
	<LocalPreGame
		controller={local}
		onGameStarted={onLocalGameStarted}
		newGameRequested={localNewGameRequested}
		onNewGameHandled={() => (localNewGameRequested = false)}
	/>
{/if}

<style>
	.play {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
	}
	.tabs {
		display: flex;
		gap: 0.25rem;
		padding: 0.35rem 1rem;
		background: rgba(0, 0, 0, 0.2);
		border-bottom: 1px solid rgba(255, 255, 255, 0.06);
	}
	:global(.app[data-skin='light']) .tabs {
		background: rgba(0, 0, 0, 0.04);
		border-bottom-color: rgba(0, 0, 0, 0.08);
	}
	.tabs button {
		background: transparent;
		color: inherit;
		border: 0;
		padding: 0.35rem 0.9rem;
		border-radius: 4px;
		font: inherit;
		cursor: pointer;
		opacity: 0.65;
	}
	.tabs button:hover:not(:disabled) {
		opacity: 1;
	}
	.tabs button.active {
		opacity: 1;
		background: rgba(246, 196, 84, 0.22);
	}
	.tabs button:disabled {
		opacity: 0.35;
		cursor: not-allowed;
	}
</style>
