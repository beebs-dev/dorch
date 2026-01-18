<script lang="ts">
	import './layout.css';
	import DorchLogo from '$lib/components/DorchLogo.svelte';
	import LoginModal from '$lib/components/LoginModal.svelte';
	import { toastMessage } from '$lib/stores/toast';
	import { base, resolve } from '$app/paths';
	import { page } from '$app/stores';
	import { browser } from '$app/environment';

	let { children } = $props();

	let loginOpen = $state(false);

	function openLogin() {
		loginOpen = true;
		if (!browser) return;
		if (window.location.hash !== '#login') {
			history.replaceState(
				history.state,
				'',
				`${window.location.pathname}${window.location.search}#login`
			);
		}
	}

	function closeLogin() {
		loginOpen = false;
		if (!browser) return;
		if (window.location.hash === '#login') {
			history.replaceState(
				history.state,
				'',
				`${window.location.pathname}${window.location.search}`
			);
		}
	}

	$effect(() => {
		if ($page.url.hash === '#login') loginOpen = true;
	});

	function isActive(href: string, pathname: string) {
		if (href === '/') return pathname === '/' || pathname.startsWith('/servers/');
		if (href === '/wad') return pathname === '/wad' || pathname.startsWith('/wad/');
		return pathname === href || pathname.startsWith(`${href}/`);
	}
</script>

<svelte:head>
	<title>GIB.GG</title>
	<link rel="icon" type="image/png" href={`${base}/favicon.png`} />
</svelte:head>

<div class="dorch-texture min-h-dvh bg-zinc-950 text-zinc-100">
	<header class="sticky top-0 z-10 border-b border-red-950/60 bg-red-950/35 backdrop-blur">
		<div class="mx-auto flex max-w-6xl flex-wrap items-center gap-x-6 gap-y-2 px-4 py-3">
			<div class="shrink-0">
				<DorchLogo />
			</div>
			<nav
				class="ml-auto flex flex-wrap items-center justify-end gap-x-6 gap-y-1"
				aria-label="Primary"
			>
				<a
					href={resolve('/')}
					aria-current={isActive('/', $page.url.pathname) ? 'page' : undefined}
					class={`-mb-px border-b-2 px-1 py-2 text-sm font-[var(--dorch-mono)] tracking-wide transition-colors focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset ${
						isActive('/', $page.url.pathname)
							? 'border-red-400 text-zinc-100'
							: 'border-transparent text-zinc-300 hover:border-red-700 hover:text-zinc-100'
					}`}
				>
					SERVERS
				</a>
				<a
					href={resolve('/wad')}
					aria-current={isActive('/wad', $page.url.pathname) ? 'page' : undefined}
					class={`-mb-px border-b-2 px-1 py-2 text-sm font-[var(--dorch-mono)] tracking-wide transition-colors focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset ${
						isActive('/wad', $page.url.pathname)
							? 'border-red-400 text-zinc-100'
							: 'border-transparent text-zinc-300 hover:border-red-700 hover:text-zinc-100'
					}`}
				>
					WAD BROWSER
				</a>
				<button
					type="button"
					class={`-mb-px border-b-2 px-1 py-2 text-sm font-[var(--dorch-mono)] tracking-wide transition-colors focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset ${
						loginOpen || $page.url.hash === '#login'
							? 'border-red-400 text-zinc-100'
							: 'border-transparent text-zinc-300 hover:border-red-700 hover:text-zinc-100'
					}`}
					onclick={openLogin}
				>
					LOGIN
				</button>
			</nav>
		</div>
	</header>
	<main class="min-w-0">{@render children()}</main>
	<LoginModal open={loginOpen} onClose={closeLogin} />
	{#if $toastMessage}
		<div
			class="fixed top-4 left-1/2 z-[999] -translate-x-1/2 rounded-md bg-zinc-900 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800"
			role="status"
			aria-live="polite"
		>
			{$toastMessage}
		</div>
	{/if}
</div>
