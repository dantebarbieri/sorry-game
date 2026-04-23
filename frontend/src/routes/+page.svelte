<script lang="ts">
	import { PIECE_MODEL_ATTRIBUTION } from '$lib/board/assets';

	const ctas = [
		{
			href: '/rules',
			title: 'Learn the rules',
			body: 'Interactive walkthrough — step through each card and trick with a live board.'
		},
		{
			href: '/play',
			title: 'Play',
			body: 'Local hot-seat, versus bots, or online over your network.'
		},
		{
			href: '/simulate',
			title: 'Simulate',
			body: 'Run thousands of games, compare strategies, and replay any match.'
		}
	];
</script>

<section class="hero">
	<h1>Sorry!</h1>
	<p class="tagline">
		A hand-made simulator and play-anywhere client for the classic board game — with
		first-class support for rules variants.
	</p>
</section>

<section class="ctas">
	{#each ctas as cta (cta.href)}
		<a class="cta" href={cta.href}>
			<h2>{cta.title}</h2>
			<p>{cta.body}</p>
		</a>
	{/each}
</section>

<footer class="attribution">
	<span>{PIECE_MODEL_ATTRIBUTION}</span>
</footer>

<style>
	.hero {
		padding: 4rem 1.5rem 2.5rem;
		max-width: 52rem;
		margin: 0 auto;
		text-align: center;
	}

	.hero h1 {
		font-size: clamp(2.5rem, 7vw, 4.5rem);
		margin: 0 0 0.5rem;
		letter-spacing: -0.02em;
	}

	.tagline {
		font-size: 1.1rem;
		opacity: 0.8;
		max-width: 36rem;
		margin: 0 auto;
		line-height: 1.5;
	}

	.ctas {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
		gap: 1rem;
		padding: 1rem 1.5rem 3rem;
		max-width: 64rem;
		margin: 0 auto;
		width: 100%;
	}

	.cta {
		display: block;
		padding: 1.5rem 1.25rem;
		border-radius: 10px;
		background: rgba(255, 255, 255, 0.04);
		border: 1px solid rgba(255, 255, 255, 0.08);
		color: inherit;
		text-decoration: none;
		transition: transform 120ms ease, background 120ms ease, border-color 120ms ease;
	}

	:global(.app[data-skin='light']) .cta {
		background: rgba(0, 0, 0, 0.03);
		border-color: rgba(0, 0, 0, 0.08);
	}

	.cta:hover,
	.cta:focus-visible {
		transform: translateY(-2px);
		background: rgba(246, 196, 84, 0.12);
		border-color: rgba(246, 196, 84, 0.5);
	}

	.cta h2 {
		margin: 0 0 0.35rem;
		font-size: 1.2rem;
	}

	.cta p {
		margin: 0;
		opacity: 0.78;
		font-size: 0.95rem;
		line-height: 1.45;
	}

	.attribution {
		padding: 1.5rem;
		text-align: center;
		font-size: 0.8rem;
		opacity: 0.55;
	}
</style>
