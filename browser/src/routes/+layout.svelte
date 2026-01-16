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
	<div class="grid min-h-dvh grid-cols-[14rem_1fr]">
		<aside class="sticky top-0 h-dvh bg-zinc-950/80 backdrop-blur" aria-label="Primary">
			<nav class="flex h-full flex-col">
				<div class="px-3 py-4">
					{#each navItems as item (item.href)}
						<a
							href={item.href}
							class={`font-[var(--dorch-mono)] block px-4 py-3 text-base tracking-wide hover:bg-zinc-900/40 ${
								isActive(item.href, $page.url.pathname) ? 'bg-zinc-900/50 text-zinc-100' : 'text-zinc-300'
							}`}
						>
							{item.label}
						</a>
					{/each}
				</div>
			</nav>
		</aside>

		<div class="min-w-0">
			<header class="sticky top-0 z-10 border-b border-zinc-900 bg-zinc-950/80 backdrop-blur">
				<div class="mx-auto flex max-w-6xl items-center gap-3 px-4 py-3">
					<div class="flex-1">
						<DorchLogo />
					</div>
					<div class="w-full max-w-[640px] flex-[2]">
						<form action="/" method="get" class="flex w-full">
							<label class="sr-only" for="global-search">Search WADs</label>
							<input
								id="global-search"
								name="q"
								value={$page.url.searchParams.get('q') ?? ''}
								placeholder="Search title, author, description, sha1…"
								class="w-full rounded-lg bg-zinc-900/60 px-3 py-2 text-sm text-zinc-100 ring-1 ring-inset ring-zinc-800 placeholder:text-zinc-500 focus:outline-none focus:ring-2 focus:ring-zinc-500"
							/>
							<button
								type="submit"
								class="ml-2 hidden rounded-lg bg-zinc-900 px-3 py-2 text-sm text-zinc-200 ring-1 ring-inset ring-zinc-800 hover:bg-zinc-800 sm:inline-block"
							>
								Search
							</button>
						</form>
					</div>
				</div>
			</header>
			<main>{@render children()}</main>
		</div>
	</div>
</div>
