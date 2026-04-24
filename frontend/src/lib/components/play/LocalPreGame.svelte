<script lang="ts">
	import { onMount } from 'svelte';
	import type { LocalController } from '$lib/play/local-controller.svelte';
	import { DEFAULT_SETUP, activeSeatSides, type PlaySetup, type ViewerSeat } from '$lib/play/types';
	import { loadSetup, saveSetup } from '$lib/play/session';
	import SetupDrawer from '$lib/components/play/SetupDrawer.svelte';

	interface Props {
		controller: LocalController;
		onGameStarted: (viewer: ViewerSeat) => void;
		/** When true (parent signals "new game"), reopen the drawer. */
		newGameRequested: boolean;
		onNewGameHandled: () => void;
	}

	const { controller, onGameStarted, newGameRequested, onNewGameHandled }: Props = $props();

	let setupOpen = $state(true);
	let initialSetupNeeded = $state(true);
	let pendingSetup = $state<PlaySetup>(DEFAULT_SETUP);

	onMount(() => {
		pendingSetup = loadSetup() ?? DEFAULT_SETUP;
	});

	$effect(() => {
		if (newGameRequested) {
			setupOpen = true;
			onNewGameHandled();
		}
	});

	function computeInitialViewer(setup: PlaySetup): ViewerSeat {
		const sides = activeSeatSides(setup);
		for (let engineIdx = 0; engineIdx < sides.length; engineIdx++) {
			if (setup.seats[sides[engineIdx]].type === 'Human') return engineIdx;
		}
		return null;
	}

	async function handleApplySetup(setup: PlaySetup) {
		saveSetup(setup);
		pendingSetup = setup;
		await controller.newGame(setup);
		const viewer = computeInitialViewer(setup);
		setupOpen = false;
		initialSetupNeeded = false;
		onGameStarted(viewer);
	}
</script>

{#if controller.gameState === null}
	<div class="pre-game">
		<p>Configure the game to begin.</p>
		<button class="primary" onclick={() => (setupOpen = true)}>Configure</button>
	</div>
{/if}

<SetupDrawer
	open={setupOpen}
	setup={initialSetupNeeded ? pendingSetup : controller.setup}
	required={initialSetupNeeded && controller.gameState === null}
	onApply={handleApplySetup}
	onClose={() => (setupOpen = false)}
/>

<style>
	.pre-game {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 1rem;
		padding: 4rem 1rem;
		opacity: 0.9;
	}
	.pre-game .primary {
		background: rgba(246, 196, 84, 0.25);
		border: 1px solid rgba(246, 196, 84, 0.7);
		color: inherit;
		padding: 0.5rem 1.25rem;
		border-radius: 6px;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
</style>
