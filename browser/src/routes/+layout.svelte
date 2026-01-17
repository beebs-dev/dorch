<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import DorchLogo from '$lib/components/DorchLogo.svelte';
	import { page } from '$app/stores';

	let { children } = $props();

	const navItems = [
		{ label: 'WAD BROWSER', href: '/' },
		{ label: 'SERVERS', href: '/servers' },
		{ label: 'PARTY', href: '/party' },
		{ label: 'SETTINGS', href: '/settings' }
	] as const;

	function isActive(href: string, pathname: string) {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<div class="min-h-dvh bg-zinc-950 text-zinc-100">
	<header class="sticky top-0 z-10 border-b border-zinc-900 bg-zinc-950/80 backdrop-blur">
		<div class="mx-auto flex max-w-6xl flex-wrap items-center gap-x-6 gap-y-2 px-4 py-3">
			<div class="shrink-0">
				<DorchLogo />
			</div>
			<nav class="ml-auto flex flex-wrap items-center justify-end gap-1" aria-label="Primary">
				{#each navItems as item (item.href)}
					<a
						href={item.href}
						aria-current={isActive(item.href, $page.url.pathname) ? 'page' : undefined}
						class={`font-[var(--dorch-mono)] rounded-lg px-3 py-2 text-sm tracking-wide ring-1 ring-inset ring-zinc-800 hover:bg-zinc-900/50 ${
							isActive(item.href, $page.url.pathname)
								? 'bg-zinc-900/60 text-zinc-100'
								: 'text-zinc-300'
						}`}
					>
						{item.label}
					</a>
				{/each}
			</nav>
		</div>
	</header>
	<main class="min-w-0">{@render children()}</main>
</div>
