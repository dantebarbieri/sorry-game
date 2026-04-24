<script lang="ts">
	import { onMount } from 'svelte';
	import {
		createRoom,
		joinRoom,
		spectateRoom,
		DEFAULT_SERVER_URL,
		type CreateRoomResponse,
		type JoinRoomResponse,
		type SpectateRoomResponse
	} from '$lib/play/rest-client';
	import type { OnlineController, OnlineRole } from '$lib/play/online-controller.svelte';
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
		role: OnlineRole;
		/** Seat index for players, spectator index for spectators. */
		index: number;
	};

	let mode = $state<'landing' | 'creating' | 'joining' | 'spectating' | 'connected'>('landing');
	let baseUrl = $state(DEFAULT_SERVER_URL);
	let playerName = $state('');
	let numPlayers = $state(4);
	let rulesChoice = $state('Standard');
	let roomCode = $state('');
	let error = $state<string | null>(null);

	onMount(async () => {
		const stored = loadSession();
		if (stored) {
			await controller.connect(
				stored.baseUrl,
				stored.code,
				stored.token,
				stored.index,
				stored.role
			);
			mode = 'connected';
			onConnected();
		}
	});

	function loadSession(): Session | null {
		if (typeof sessionStorage === 'undefined') return null;
		const raw = sessionStorage.getItem(SESSION_KEY);
		if (!raw) return null;
		try {
			const parsed = JSON.parse(raw) as Partial<Session> & { playerIndex?: number };
			// Back-compat: previous versions stored { playerIndex } without role.
			if (!parsed.role) {
				return {
					baseUrl: parsed.baseUrl ?? DEFAULT_SERVER_URL,
					code: parsed.code ?? '',
					token: parsed.token ?? '',
					role: 'player',
					index: parsed.playerIndex ?? parsed.index ?? 0
				};
			}
			return parsed as Session;
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
			const session: Session = {
				baseUrl,
				code: res.room_code,
				token: res.session_token,
				role: 'player',
				index: res.player_index
			};
			saveSession(session);
			await controller.connect(baseUrl, res.room_code, res.session_token, res.player_index, 'player');
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
			const session: Session = {
				baseUrl,
				code,
				token: res.session_token,
				role: 'player',
				index: res.player_index
			};
			saveSession(session);
			await controller.connect(baseUrl, code, res.session_token, res.player_index, 'player');
			mode = 'connected';
			onConnected();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			mode = 'landing';
		}
	}

	async function doSpectate() {
		if (!playerName.trim() || !roomCode.trim()) {
			error = 'Enter name and room code';
			return;
		}
		error = null;
		mode = 'spectating';
		try {
			const code = roomCode.trim().toUpperCase();
			const res: SpectateRoomResponse = await spectateRoom(
				code,
				{ player_name: playerName.trim() },
				baseUrl
			);
			const session: Session = {
				baseUrl,
				code,
				token: res.session_token,
				role: 'spectator',
				index: res.spectator_index
			};
			saveSession(session);
			await controller.connect(baseUrl, code, res.session_token, res.spectator_index, 'spectator');
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

	function onBecomeSpectator() {
		controller.becomeSpectator();
		// Rewrite session so a reload comes back as a spectator.
		const lobby = controller.lobby;
		const stored = loadSession();
		if (stored && lobby) {
			saveSession({ ...stored, role: 'spectator', index: lobby.spectators.length });
		}
	}

	function onTakeSlot(slot: number) {
		controller.takeSlot(slot);
		// For seated players this doesn't change role; the server will
		// reply with an updated RoomState. Update stored seat index so a
		// reload targets the correct slot.
		const stored = loadSession();
		if (stored && stored.role === 'player') {
			saveSession({ ...stored, index: slot });
		}
	}

	const lobby = $derived(controller.lobby);
	const isHost = $derived(
		lobby !== null && controller.role === 'player' && controller.viewer === lobby.creator
	);
	const phase = $derived(lobby?.phase ?? 'unknown');
	const isSpectator = $derived(controller.role === 'spectator');
	const mySeat = $derived(controller.role === 'player' ? controller.viewer : null);
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

		<div class="three-col">
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

			<fieldset>
				<legend>Spectate</legend>
				<p class="hint">Watch any room — no seat required.</p>
				<button class="primary" onclick={doSpectate} disabled={mode !== 'landing'}>
					Spectate
				</button>
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
				{#if isSpectator}
					<span class="badge-spec" title="You are spectating">Spectating</span>
				{/if}
				{#if isHost && phase === 'lobby'}
					<button class="primary" onclick={() => controller.startGame()}>Start game</button>
				{/if}
				{#if !isSpectator && phase === 'lobby' && mySeat !== null}
					<button onclick={onBecomeSpectator}>Become spectator</button>
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
				<div class="slot" class:you={player.slot === mySeat}>
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
					{#if phase === 'lobby' && player.player_type.kind !== 'Human' && player.slot !== mySeat}
						<button class="take" onclick={() => onTakeSlot(player.slot)}>
							Take this color
						</button>
					{/if}
					{#if isHost && phase === 'lobby' && player.slot !== mySeat}
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
						{#if player.player_type.kind === 'Human'}
							<button onclick={() => controller.kickPlayer(player.slot)} class="kick">
								Kick
							</button>
						{/if}
					{/if}
				</div>
			{/each}
		</section>

		<section class="spectators">
			<h3>
				Spectators
				<span class="count">{lobby.spectators.length}</span>
			</h3>
			{#if lobby.spectators.length === 0}
				<p class="hint">No spectators yet.</p>
			{:else}
				<ul>
					{#each lobby.spectators as spec (spec.idx)}
						<li class:you={isSpectator && spec.idx === controller.viewer}>
							<span class="name">{spec.name || 'Anonymous'}</span>
							{#if !spec.connected}<em class="dim">disconnected</em>{/if}
						</li>
					{/each}
				</ul>
			{/if}
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
	.three-col {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 1rem;
	}
	@media (max-width: 40rem) {
		.three-col {
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
	button.take {
		font-size: 0.75rem;
		padding: 0.2rem 0.5rem;
		background: rgba(246, 196, 84, 0.12);
		border-color: rgba(246, 196, 84, 0.5);
	}
	.badge-spec {
		font-size: 0.75rem;
		padding: 0.2rem 0.5rem;
		border-radius: 4px;
		background: rgba(120, 180, 255, 0.18);
		border: 1px solid rgba(120, 180, 255, 0.5);
	}
	.spectators {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		padding-top: 0.5rem;
		border-top: 1px solid rgba(255, 255, 255, 0.08);
	}
	:global(.app[data-skin='light']) .spectators {
		border-top-color: rgba(0, 0, 0, 0.08);
	}
	.spectators h3 {
		margin: 0;
		font-size: 0.85rem;
		font-weight: 600;
		opacity: 0.75;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.spectators .count {
		font-size: 0.75rem;
		opacity: 0.6;
	}
	.spectators ul {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}
	.spectators li {
		padding: 0.2rem 0.55rem;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.04);
		font-size: 0.85rem;
	}
	:global(.app[data-skin='light']) .spectators li {
		background: rgba(0, 0, 0, 0.03);
	}
	.spectators li.you {
		outline: 1px solid rgba(246, 196, 84, 0.5);
	}
	.spectators .dim {
		opacity: 0.6;
		margin-left: 0.35rem;
		font-style: italic;
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
