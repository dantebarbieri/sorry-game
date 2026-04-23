<script lang="ts">
	import { onMount } from 'svelte';
	import {
		createRoom,
		joinRoom,
		DEFAULT_SERVER_URL,
		type CreateRoomResponse,
		type JoinRoomResponse
	} from '$lib/play/rest-client';
	import type { OnlineController } from '$lib/play/online-controller.svelte';
	import { PLAYER_NAMES } from '$lib/play/types';
	import { useTheme } from '$lib/theme-context.svelte';

	interface Props {
		controller: OnlineController;
		/** Emitted once we have a session and the WS is connected. */
		onConnected: () => void;
	}

	const { controller, onConnected }: Props = $props();

	const theme = useTheme();

	const SESSION_KEY = 'sorry:online-session';
	type Session = {
		baseUrl: string;
		code: string;
		token: string;
		playerIndex: number;
	};

	let mode = $state<'landing' | 'creating' | 'joining' | 'connected'>('landing');
	let baseUrl = $state(DEFAULT_SERVER_URL);
	let playerName = $state('');
	let numPlayers = $state(4);
	let rulesChoice = $state('Standard');
	let roomCode = $state('');
	let error = $state<string | null>(null);

	onMount(async () => {
		const stored = loadSession();
		if (stored) {
			await controller.connect(stored.baseUrl, stored.code, stored.token, stored.playerIndex);
			mode = 'connected';
			onConnected();
		}
	});

	function loadSession(): Session | null {
		if (typeof sessionStorage === 'undefined') return null;
		const raw = sessionStorage.getItem(SESSION_KEY);
		if (!raw) return null;
		try {
			return JSON.parse(raw) as Session;
		} catch {
			return null;
		}
	}

	function saveSession(session: Session) {
		if (typeof sessionStorage === 'undefined') return;
		sessionStorage.setItem(SESSION_KEY, JSON.stringify(session));
	}

	function clearSession() {
		if (typeof sessionStorage === 'undefined') return;
		sessionStorage.removeItem(SESSION_KEY);
	}

	async function doCreate() {
		if (!playerName.trim()) {
			error = 'Enter a name';
			return;
		}
		error = null;
		mode = 'creating';
		try {
			const res: CreateRoomResponse = await createRoom(
				{ player_name: playerName.trim(), num_players: numPlayers, rules: rulesChoice },
				baseUrl
			);
			const session = {
				baseUrl,
				code: res.room_code,
				token: res.session_token,
				playerIndex: res.player_index
			};
			saveSession(session);
			await controller.connect(baseUrl, res.room_code, res.session_token, res.player_index);
			mode = 'connected';
			onConnected();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			mode = 'landing';
		}
	}

	async function doJoin() {
		if (!playerName.trim() || !roomCode.trim()) {
			error = 'Enter name and room code';
			return;
		}
		error = null;
		mode = 'joining';
		try {
			const code = roomCode.trim().toUpperCase();
			const res: JoinRoomResponse = await joinRoom(code, { player_name: playerName.trim() }, baseUrl);
			const session = {
				baseUrl,
				code,
				token: res.session_token,
				playerIndex: res.player_index
			};
			saveSession(session);
			await controller.connect(baseUrl, code, res.session_token, res.player_index);
			mode = 'connected';
			onConnected();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			mode = 'landing';
		}
	}

	function disconnect() {
		clearSession();
		controller.disconnect();
		mode = 'landing';
	}

	function copyCode() {
		const code = controller.lobby?.room_code;
		if (code) void navigator.clipboard?.writeText(code);
	}

	const lobby = $derived(controller.lobby);
	const isHost = $derived(lobby !== null && controller.viewer === lobby.creator);
	const phase = $derived(lobby?.phase ?? 'unknown');
</script>

{#if mode !== 'connected'}
	<div class="landing">
		<h2>Online play</h2>
		<p class="hint">
			Create or join a room served by a local <code>sorry-server</code> (default:
			<code>{DEFAULT_SERVER_URL}</code>).
		</p>
		<label>
			Server URL
			<input type="text" bind:value={baseUrl} placeholder="http://localhost:3030" />
		</label>
		<label>
			Your name
			<input type="text" bind:value={playerName} placeholder="Name" />
		</label>

		<div class="two-col">
			<fieldset>
				<legend>Create a room</legend>
				<label>
					Players
					<select bind:value={numPlayers}>
						<option value={2}>2</option>
						<option value={3}>3</option>
						<option value={4}>4</option>
					</select>
				</label>
				<label>
					Rules
					<select bind:value={rulesChoice}>
						<option value="Standard">Standard</option>
						<option value="PlayOut">PlayOut</option>
					</select>
				</label>
				<button class="primary" onclick={doCreate} disabled={mode !== 'landing'}>Create</button>
			</fieldset>

			<fieldset>
				<legend>Join a room</legend>
				<label>
					Room code
					<input
						type="text"
						bind:value={roomCode}
						placeholder="e.g. A1B2C"
						maxlength="12"
						style:text-transform="uppercase"
					/>
				</label>
				<button class="primary" onclick={doJoin} disabled={mode !== 'landing'}>Join</button>
			</fieldset>
		</div>

		{#if error}
			<p class="err">{error}</p>
		{/if}
	</div>
{:else if lobby}
	<div class="lobby">
		<header>
			<div>
				<button class="room-code" onclick={copyCode} title="Click to copy room code" type="button">
					{lobby.room_code}
				</button>
				<span class="phase">{phase}</span>
			</div>
			<div class="lobby-actions">
				{#if isHost && phase === 'lobby'}
					<button class="primary" onclick={() => controller.startGame()}>Start game</button>
				{/if}
				{#if phase === 'post-game'}
					<button class="primary" onclick={() => controller.newGame()}>Play again</button>
					<button onclick={() => controller.returnToLobby()}>Back to lobby</button>
				{/if}
				<button onclick={disconnect}>Leave</button>
			</div>
		</header>

		<section class="slots">
			{#each lobby.players as player (player.slot)}
				<div class="slot" class:you={player.slot === controller.viewer}>
					<span
						class="dot"
						style:background={theme.skin.palette.players[player.slot]}
						aria-hidden="true"
					></span>
					<span class="seat">{PLAYER_NAMES[player.slot] ?? `P${player.slot}`}</span>
					<span class="name">{player.name}</span>
					<span class="type">
						{#if player.player_type.kind === 'Human'}
							Human{#if !player.connected} · <em>disconnected</em>{/if}
						{:else if player.player_type.kind === 'Bot'}
							Bot · {player.player_type.strategy}
						{:else}
							<em>Empty</em>
						{/if}
					</span>
					{#if isHost && phase === 'lobby' && player.slot !== controller.viewer}
						<select
							class="type-picker"
							value={player.player_type.kind === 'Bot' ? `Bot:${player.player_type.strategy}` : player.player_type.kind}
							onchange={(e) => {
								const val = (e.currentTarget as HTMLSelectElement).value;
								if (val === 'Human') {
									controller.configureSlot(player.slot, JSON.stringify({ kind: 'Human' }));
								} else if (val === 'Empty') {
									controller.configureSlot(player.slot, JSON.stringify({ kind: 'Empty' }));
								} else if (val.startsWith('Bot:')) {
									const strat = val.slice('Bot:'.length);
									controller.configureSlot(
										player.slot,
										JSON.stringify({ kind: 'Bot', strategy: strat })
									);
								}
							}}
						>
							<option value="Human">Human (open seat)</option>
							<option value="Empty">Empty</option>
							{#each lobby.available_strategies as s (s)}
								<option value={'Bot:' + s}>Bot · {s}</option>
							{/each}
						</select>
						<button onclick={() => controller.kickPlayer(player.slot)} class="kick">
							Kick
						</button>
					{/if}
				</div>
			{/each}
		</section>

		{#if controller.error}
			<p class="err">{controller.error}</p>
		{/if}
	</div>
{:else}
	<p class="connecting">Connecting…</p>
{/if}

<style>
	.landing {
		max-width: 36rem;
		margin: 0 auto;
		padding: 1.5rem 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	.landing h2 {
		margin: 0;
	}
	.hint {
		font-size: 0.85rem;
		opacity: 0.7;
		margin: 0 0 0.25rem;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.8rem;
	}
	input,
	select {
		background: rgba(255, 255, 255, 0.06);
		color: inherit;
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 4px;
		padding: 0.35rem 0.5rem;
		font: inherit;
	}
	:global(.app[data-skin='light']) input,
	:global(.app[data-skin='light']) select {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.12);
	}
	.two-col {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
	}
	@media (max-width: 36rem) {
		.two-col {
			grid-template-columns: 1fr;
		}
	}
	fieldset {
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 6px;
		padding: 0.75rem;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	:global(.app[data-skin='light']) fieldset {
		border-color: rgba(0, 0, 0, 0.12);
	}
	legend {
		font-size: 0.78rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
		padding: 0 0.35rem;
	}
	button {
		background: transparent;
		color: inherit;
		border: 1px solid currentColor;
		padding: 0.35rem 0.8rem;
		border-radius: 4px;
		cursor: pointer;
		font: inherit;
		opacity: 0.8;
	}
	button:hover:not(:disabled) {
		opacity: 1;
	}
	button.primary {
		background: rgba(246, 196, 84, 0.25);
		border-color: rgba(246, 196, 84, 0.7);
		font-weight: 600;
	}
	button.kick {
		font-size: 0.75rem;
		padding: 0.2rem 0.5rem;
	}
	.err {
		color: salmon;
		font-size: 0.9rem;
	}

	.lobby {
		max-width: 48rem;
		margin: 0 auto;
		padding: 1.25rem 1rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.lobby header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
		flex-wrap: wrap;
	}
	.room-code {
		font-family: ui-monospace, monospace;
		font-size: 1.4rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		padding: 0.2rem 0.8rem;
		border-radius: 4px;
		background: rgba(246, 196, 84, 0.2);
		border: 0;
		color: inherit;
		cursor: pointer;
	}
	.phase {
		font-size: 0.8rem;
		opacity: 0.7;
		margin-left: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}
	.lobby-actions {
		display: flex;
		gap: 0.4rem;
		flex-wrap: wrap;
	}
	.slots {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}
	.slot {
		display: grid;
		grid-template-columns: auto auto 1fr auto auto auto;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.75rem;
		border-radius: 6px;
		background: rgba(255, 255, 255, 0.03);
	}
	:global(.app[data-skin='light']) .slot {
		background: rgba(0, 0, 0, 0.03);
	}
	.slot.you {
		outline: 1px solid rgba(246, 196, 84, 0.5);
	}
	.dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
	}
	.seat {
		font-weight: 600;
		min-width: 3.5rem;
	}
	.name {
		opacity: 0.9;
	}
	.type {
		opacity: 0.75;
		font-size: 0.85rem;
	}
	.type-picker {
		font-size: 0.8rem;
	}
	.connecting {
		text-align: center;
		padding: 2rem;
		opacity: 0.7;
	}
</style>
