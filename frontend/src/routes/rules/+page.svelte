<script lang="ts">
	import CardDemo from '$lib/components/rules/CardDemo.svelte';
	import {
		DEMO_SETUP,
		DEMO_ONE,
		DEMO_FOUR,
		DEMO_SEVEN_SPLIT,
		DEMO_ELEVEN_SWAP,
		DEMO_SORRY
	} from '$lib/rules/demo-specs';

	const sections = [
		{ id: 'overview', label: 'Overview' },
		{ id: 'setup', label: 'Setup' },
		{ id: 'turn', label: 'Your turn' },
		{ id: 'cards', label: 'Cards' },
		{ id: 'slides', label: 'Slides' },
		{ id: 'sorry', label: 'Sorry!' },
		{ id: 'winning', label: 'Winning' },
		{ id: 'strategies', label: 'Strategies' }
	];
</script>

<div class="rules">
	<aside class="toc" aria-label="Table of contents">
		<h2>Rules</h2>
		<nav>
			<ul>
				{#each sections as section (section.id)}
					<li><a href="#{section.id}">{section.label}</a></li>
				{/each}
			</ul>
		</nav>
		<p class="kbd-hint">
			Click or Tab to a demo, then <kbd>←</kbd> / <kbd>→</kbd> to step and
			<kbd>Home</kbd> to reset.
		</p>
	</aside>

	<main class="content">
		<section id="overview">
			<h2>Overview</h2>
			<p>
				<strong>Sorry!</strong> is a classic race-home board game for 2–4 players. Each player
				controls four pawns of their color, drawing one card per turn and moving according to
				its value. The twist: some cards send opponents back to Start, and slides can catapult
				you (or a pawn in your way) across the board.
			</p>
			<p>
				The first player to bring all four pawns <em>home</em> wins. In the <strong
					>Play Out</strong
				>
				variant, play continues until every placement — 1st through 4th — has been decided.
			</p>
		</section>

		<section id="setup">
			<h2>Setup</h2>
			<p>
				Each player's four pawns start on their color's <em>Start</em> area. Colors go around
				the board clockwise — Red, Blue, Yellow, Green. The deck is shuffled once at the
				beginning; when the deck runs out, the discard pile is reshuffled.
			</p>
			<CardDemo spec={DEMO_SETUP} />
		</section>

		<section id="turn">
			<h2>Your turn</h2>
			<ol>
				<li>Draw one card from the top of the deck.</li>
				<li>Play it according to its rules — you <em>must</em> move if any legal move exists.</li>
				<li>
					If no legal move is available, you pass. (A 2 grants an extra turn; a Sorry! or 11 may
					not always be playable.)
				</li>
			</ol>
		</section>

		<section id="cards">
			<h2>Cards</h2>
			<p>
				Each card in the deck has its own meaning. Here are the ones with unique behavior — the
				rest (3, 5, 8, 12) simply move a pawn forward by their value.
			</p>

			<CardDemo spec={DEMO_ONE} />
			<CardDemo spec={DEMO_FOUR} />
			<CardDemo spec={DEMO_SEVEN_SPLIT} />
			<CardDemo spec={DEMO_ELEVEN_SWAP} />

			<h3>Other cards</h3>
			<ul class="card-list">
				<li><strong>2</strong> — move 2 spaces; grants an extra turn.</li>
				<li><strong>3</strong> — move 3 spaces forward.</li>
				<li><strong>5</strong> — move 5 spaces forward.</li>
				<li><strong>8</strong> — move 8 spaces forward.</li>
				<li><strong>10</strong> — move 10 forward, <em>or</em> 1 backward.</li>
				<li><strong>12</strong> — move 12 spaces forward.</li>
			</ul>
		</section>

		<section id="slides">
			<h2>Slides</h2>
			<p>
				Each color has two slide tracks embedded in the main loop. If your pawn lands on the
				head of any slide — except one of your own — you slide all the way to the end. Any pawn
				(including yours) caught on the slide's path is sent to their Start.
			</p>
			<p class="hint">
				Slides are the fastest way around the board, but landing on your own slide's head does
				nothing — the game rewards careful counting of spaces.
			</p>
		</section>

		<section id="sorry">
			<h2>Sorry!</h2>
			<p>
				A <strong>Sorry!</strong> card teleports one pawn from your Start area directly onto any
				opponent pawn on the main track, sending that pawn back to their Start. If no opponent
				is vulnerable and you have no pawn in Start, the card is forfeited.
			</p>
			<CardDemo spec={DEMO_SORRY} />
		</section>

		<section id="winning">
			<h2>Winning</h2>
			<p>
				Once a pawn reaches its color's <em>Home</em> zone, it's safe — no card can send it
				back. The first player to bring all four pawns home wins the game. In the Play Out
				variant, subsequent players continue playing for 2nd, 3rd, and 4th place.
			</p>
		</section>

		<section id="strategies">
			<h2>Strategies</h2>
			<p>
				The simulator page lets you run thousands of games against different computer
				strategies. Here are the ones planned for this project:
			</p>
			<ul class="strategy-list">
				<li>
					<strong>Random</strong> <span class="tag">shipped</span> — picks a legal move uniformly
					at random. Useful as a baseline.
				</li>
				<li>
					<strong>Greedy</strong> <span class="tag">planned</span> — always tries to minimize
					the distance of its pawns to Home.
				</li>
				<li>
					<strong>Not Sorry</strong> <span class="tag">planned</span> — prioritizes getting
					pawns out of Start; falls back to greedy.
				</li>
				<li>
					<strong>Survivor</strong> <span class="tag">planned</span> — attacks the current
					leader at any cost, even sacrificing its own position.
				</li>
				<li>
					<strong>Reverse</strong> <span class="tag">planned</span> — hangs a pawn just past its
					start exit, hoping to cascade home on a single 4 or a chain of 10s.
				</li>
				<li>
					<strong>Teleporter</strong> <span class="tag">planned</span> — always takes an 11
					swap when legal, picking the target that advances it furthest.
				</li>
				<li>
					<strong>Sidekick</strong> <span class="tag">planned</span> — piles on whichever
					opponent most recently lost a pawn.
				</li>
			</ul>
		</section>
	</main>
</div>

<style>
	.rules {
		display: grid;
		grid-template-columns: minmax(12rem, 16rem) 1fr;
		gap: 2rem;
		max-width: 72rem;
		margin: 0 auto;
		padding: 2rem 1.5rem;
		width: 100%;
	}
	@media (max-width: 48rem) {
		.rules {
			grid-template-columns: 1fr;
		}
	}
	.toc {
		position: sticky;
		top: 4rem;
		align-self: start;
	}
	.toc h2 {
		margin: 0 0 0.75rem;
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		opacity: 0.65;
	}
	.toc ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}
	.toc a {
		color: inherit;
		text-decoration: none;
		display: block;
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
		opacity: 0.75;
	}
	.toc a:hover {
		opacity: 1;
		background: rgba(255, 255, 255, 0.06);
	}
	:global(.app[data-skin='light']) .toc a:hover {
		background: rgba(0, 0, 0, 0.05);
	}
	.kbd-hint {
		font-size: 0.78rem;
		opacity: 0.6;
		margin-top: 1rem;
	}
	kbd {
		font-family: ui-monospace, monospace;
		background: rgba(255, 255, 255, 0.08);
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 3px;
		padding: 0.05rem 0.3rem;
		font-size: 0.85em;
	}
	:global(.app[data-skin='light']) kbd {
		background: rgba(0, 0, 0, 0.06);
		border-color: rgba(0, 0, 0, 0.15);
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: 2.25rem;
		min-width: 0;
	}
	section h2 {
		margin: 0 0 0.75rem;
		font-size: 1.5rem;
		letter-spacing: -0.01em;
	}
	section h3 {
		margin: 1rem 0 0.5rem;
	}
	section p,
	section li {
		line-height: 1.55;
	}
	.hint {
		opacity: 0.75;
		font-style: italic;
	}
	.card-list,
	.strategy-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}
	.strategy-list li {
		padding: 0.5rem 0.75rem;
		background: rgba(255, 255, 255, 0.03);
		border-radius: 5px;
	}
	:global(.app[data-skin='light']) .strategy-list li {
		background: rgba(0, 0, 0, 0.03);
	}
	.tag {
		display: inline-block;
		font-size: 0.7rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		padding: 0.05rem 0.4rem;
		border-radius: 3px;
		background: rgba(246, 196, 84, 0.25);
		margin: 0 0.35rem;
	}
</style>
