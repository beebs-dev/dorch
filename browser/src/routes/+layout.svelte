<script lang="ts">
	import './layout.css';
	import DorchLogo from '$lib/components/DorchLogo.svelte';
	import LoginModal from '$lib/components/LoginModal.svelte';
	import { toastMessage } from '$lib/stores/toast';
	import { base, resolve } from '$app/paths';
	import { page } from '$app/stores';
	import { browser } from '$app/environment';
	import { replaceState } from '$app/navigation';
	import { onMount } from 'svelte';

	let { children } = $props();

	let loginOpen = $state(false);

	function syncLoginFromUrl() {
		if (!browser) return;
		loginOpen = window.location.hash === '#login';
	}

	onMount(() => {
		if (!browser) return;
		syncLoginFromUrl();

		window.addEventListener('hashchange', syncLoginFromUrl);
		window.addEventListener('popstate', syncLoginFromUrl);
		return () => {
			window.removeEventListener('hashchange', syncLoginFromUrl);
			window.removeEventListener('popstate', syncLoginFromUrl);
		};
	});

	function openLogin() {
		loginOpen = true;
		if (!browser) return;
		if (window.location.hash !== '#login') {
			replaceState(`${$page.url.pathname}${$page.url.search}#login`, {});
		}
	}

	function closeLogin() {
		loginOpen = false;
		if (!browser) return;
		if (window.location.hash === '#login') {
			replaceState(`${$page.url.pathname}${$page.url.search}`, {});
		}
	}

	function isActive(href: string, pathname: string) {
		if (href === '/') return pathname === '/' || pathname.startsWith('/servers/');
		if (href === '/wad') return pathname === '/wad' || pathname.startsWith('/wad/');
		if (href === '/account') return pathname === '/account' || pathname.startsWith('/account/');
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	async function signOut() {
		if (!browser) return;
		try {
			await fetch(resolve('/api/logout'), { method: 'POST' });
		} catch {
			// ignore
		}

		const home = resolve('/');
		if ($page.url.pathname === home) {
			window.location.reload();
			return;
		}
		window.location.assign(home);
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
				{#if $page.data.loggedIn}
					<div class="relative -mb-px group">
						<a
							href={resolve('/account')}
							aria-current={isActive('/account', $page.url.pathname) ? 'page' : undefined}
							class={`inline-flex items-center gap-1 border-b-2 px-1 py-2 text-sm font-[var(--dorch-mono)] tracking-wide transition-colors focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset ${
								isActive('/account', $page.url.pathname)
									? 'border-red-400 text-zinc-100'
									: 'border-transparent text-zinc-300 hover:border-red-700 hover:text-zinc-100'
							}`}
						>
							ACCOUNT
							<svg
								class="h-4 w-4 opacity-80"
								viewBox="0 0 20 20"
								fill="currentColor"
								aria-hidden="true"
							>
								<path
									fill-rule="evenodd"
									d="M5.23 7.21a.75.75 0 0 1 1.06.02L10 10.94l3.71-3.71a.75.75 0 1 1 1.06 1.06l-4.24 4.25a.75.75 0 0 1-1.06 0L5.21 8.29a.75.75 0 0 1 .02-1.08Z"
									clip-rule="evenodd"
								/>
							</svg>
						</a>

						<div
							class="absolute right-0 top-full z-50 hidden min-w-48 pt-2 group-hover:block group-focus-within:block"
						>
							<div class="overflow-hidden rounded-lg bg-zinc-950 ring-1 ring-zinc-800">
								<a
									href={resolve('/account')}
									class="block px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-900 focus-visible:bg-zinc-900 focus-visible:outline-none"
								>
									Manage Account
								</a>
								<button
									type="button"
									class="w-full text-left px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-900 focus-visible:bg-zinc-900 focus-visible:outline-none"
									onclick={signOut}
								>
									Sign out
								</button>
							</div>
						</div>
					</div>
				{:else}
					<button
						type="button"
						class={`cursor-pointer -mb-px border-b-2 px-1 py-2 text-sm font-[var(--dorch-mono)] tracking-wide transition-colors focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset ${
							loginOpen
								? 'border-red-400 text-zinc-100'
								: 'border-transparent text-zinc-300 hover:border-red-700 hover:text-zinc-100'
						}`}
						onclick={openLogin}
					>
						LOGIN
					</button>
				{/if}
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
