<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { page } from '$app/stores';
	import {
		createThemeStore,
		loadStoredSkin,
		provideTheme,
		type ThemeStore
	} from '$lib/theme-context.svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const theme: ThemeStore = provideTheme(createThemeStore());

	onMount(() => {
		theme.skin = loadStoredSkin();
	});

	const navLinks = [
		{ href: '/', label: 'Home' },
		{ href: '/rules', label: 'Rules' },
		{ href: '/play', label: 'Play' },
		{ href: '/simulate', label: 'Simulate' }
	];

	function isActive(href: string, pathname: string): boolean {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(href + '/');
	}
</script>

<svelte:head>
	<title>Sorry!</title>
</svelte:head>

<div class="app" data-skin={theme.skin.id}>
	<header class="nav">
		<a class="brand" href="/">Sorry!</a>
		<nav aria-label="Primary">
			{#each navLinks as link (link.href)}
				<a
					href={link.href}
					class:active={isActive(link.href, $page.url.pathname)}
					aria-current={isActive(link.href, $page.url.pathname) ? 'page' : undefined}
				>
					{link.label}
				</a>
			{/each}
		</nav>
		<button
			type="button"
			class="skin-toggle"
			onclick={() => theme.toggle()}
			aria-label={theme.skin.id === 'light' ? 'Switch to dark skin' : 'Switch to light skin'}
			title={theme.skin.id === 'light' ? 'Switch to dark skin' : 'Switch to light skin'}
		>
			{theme.skin.id === 'light' ? 'Dark' : 'Light'}
		</button>
	</header>

	<main>
		{@render children()}
	</main>
</div>

<style>
	:global(html, body) {
		margin: 0;
		padding: 0;
		background: #0e141c;
		color: #e8ecf2;
		font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
	}

	:global(*) {
		box-sizing: border-box;
	}

	:global(:focus-visible) {
		outline: 2px solid #f6c454;
		outline-offset: 2px;
		border-radius: 2px;
	}

	.app {
		min-height: 100vh;
		display: flex;
		flex-direction: column;
	}

	.app[data-skin='light'] {
		background: #f4efde;
		color: #23201a;
	}

	.nav {
		position: sticky;
		top: 0;
		z-index: 50;
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.65rem 1.25rem;
		background: rgba(14, 20, 28, 0.88);
		backdrop-filter: blur(8px);
		border-bottom: 1px solid rgba(255, 255, 255, 0.08);
	}

	.app[data-skin='light'] .nav {
		background: rgba(244, 239, 222, 0.88);
		border-bottom-color: rgba(0, 0, 0, 0.08);
	}

	.brand {
		font-weight: 700;
		font-size: 1.1rem;
		letter-spacing: 0.02em;
		color: inherit;
		text-decoration: none;
	}

	.nav nav {
		display: flex;
		gap: 0.25rem;
		flex: 1;
	}

	.nav nav a {
		color: inherit;
		text-decoration: none;
		padding: 0.35rem 0.7rem;
		border-radius: 6px;
		font-size: 0.95rem;
		opacity: 0.75;
	}

	.nav nav a:hover {
		opacity: 1;
		background: rgba(255, 255, 255, 0.06);
	}

	.app[data-skin='light'] .nav nav a:hover {
		background: rgba(0, 0, 0, 0.06);
	}

	.nav nav a.active {
		opacity: 1;
		background: rgba(246, 196, 84, 0.18);
	}

	.skin-toggle {
		background: transparent;
		border: 1px solid currentColor;
		color: inherit;
		padding: 0.3rem 0.7rem;
		border-radius: 6px;
		font-size: 0.85rem;
		cursor: pointer;
		opacity: 0.75;
	}

	.skin-toggle:hover {
		opacity: 1;
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
	}
</style>
