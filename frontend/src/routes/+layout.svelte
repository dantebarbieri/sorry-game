<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/stores';
	import {
		createThemeStore,
		provideTheme,
		type ThemeMode,
		type ThemeStore
	} from '$lib/theme-context.svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const theme: ThemeStore = provideTheme(createThemeStore());

	const navLinks = [
		{ href: '/', label: 'Home' },
		{ href: '/rules', label: 'Rules' },
		{ href: '/play', label: 'Play' },
		{ href: '/simulate', label: 'Simulate' }
	];

	const themeOptions: { mode: ThemeMode; label: string }[] = [
		{ mode: 'light', label: 'Light' },
		{ mode: 'dark', label: 'Dark' },
		{ mode: 'auto', label: 'Auto' }
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
		<div class="skin-segmented" role="group" aria-label="Theme">
			{#each themeOptions as opt (opt.mode)}
				<button
					type="button"
					class="skin-option"
					aria-pressed={theme.mode === opt.mode}
					onclick={() => theme.setMode(opt.mode)}
					title={opt.mode === 'auto' ? 'Follow system preference' : `Use ${opt.label.toLowerCase()} skin`}
				>
					{opt.label}
				</button>
			{/each}
		</div>
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

	.skin-segmented {
		display: inline-flex;
		border: 1px solid currentColor;
		border-radius: 6px;
		overflow: hidden;
		opacity: 0.75;
	}

	.skin-option {
		background: transparent;
		color: inherit;
		border: 0;
		padding: 0.3rem 0.6rem;
		font: inherit;
		font-size: 0.85rem;
		cursor: pointer;
		border-right: 1px solid currentColor;
	}

	.skin-option:last-child {
		border-right: 0;
	}

	.skin-option:hover {
		background: rgba(255, 255, 255, 0.08);
	}

	.app[data-skin='light'] .skin-option:hover {
		background: rgba(0, 0, 0, 0.06);
	}

	.skin-option[aria-pressed='true'] {
		background: rgba(246, 196, 84, 0.22);
		font-weight: 600;
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
	}
</style>
