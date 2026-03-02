<script lang="ts">
	import type { PageData } from './$types';
	import { goto, invalidateAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { getAccessToken, subscribe as authSubscribe } from '$lib/stores/auth';
	import { onMount } from 'svelte';

	let { data }: { data: PageData } = $props();

	let notAuthenticated = $state<boolean>(data.notAuthenticated);
	let authChecking = $state<boolean>(data.notAuthenticated);
	let errorMessage = $state<string | null>(data.errorMessage ?? null);
	let refreshing = $state(false);

	const rows = $derived(() => data.rows ?? []);

	$effect(() => {
		notAuthenticated = data.notAuthenticated;
		errorMessage = data.errorMessage ?? null;
		if (!data.notAuthenticated) {
			authChecking = false;
		}
	});

	async function refresh() {
		if (refreshing) return;
		refreshing = true;
		try {
			await invalidateAll();
		} finally {
			refreshing = false;
		}
	}

	async function openGame(gameId: string) {
		await goto(resolve(`/servers/${encodeURIComponent(gameId)}`));
	}

	async function onRowKeyDown(e: KeyboardEvent, gameId: string) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			await openGame(gameId);
		}
	}

	function visibilityLabel(isPrivate: boolean | undefined): string {
		return isPrivate ? 'Private' : 'Public';
	}

	onMount(() => {
		// If SSR returned notAuthenticated, try to refresh the token
		// (the access token cookie may have expired but refresh token in localStorage is still valid)
		if (data.notAuthenticated) {
			getAccessToken()
				.then((token) => {
					if (token) {
						invalidateAll().then(() => {
							authChecking = false;
							notAuthenticated = false;
						});
					} else {
						authChecking = false;
					}
				})
				.catch(() => {
					authChecking = false;
				});
		} else {
			authChecking = false;
		}

		const unsubscribeAuth = authSubscribe((state) => {
			if (authChecking) return;
			if (!state.isAuthenticated) {
				notAuthenticated = true;
			}
		});

		return () => {
			unsubscribeAuth();
		};
	});
</script>

<svelte:head>
	<title>MY SERVERS - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<div class="flex flex-wrap items-end justify-between gap-4">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">My Servers</h1>
			<div class="mt-1 text-sm text-zinc-400">Servers you created</div>
		</div>
		<div class="flex items-center gap-2">
			<button
				type="button"
				class="rounded-md bg-zinc-900 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none disabled:opacity-50"
				onclick={refresh}
				disabled={refreshing || notAuthenticated}
				aria-busy={refreshing}
			>
				{refreshing ? 'Refreshing…' : 'Refresh'}
			</button>
		</div>
	</div>

	{#if authChecking}
		<div class="flex items-center justify-center py-12">
			<div
				class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-red-500"
			></div>
		</div>
	{:else if notAuthenticated}
		<div class="mt-6 rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="mb-4 text-zinc-400">Please log in to manage your servers.</p>
			<a
				href="/#login"
				class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-600"
			>
				Log In
			</a>
		</div>
	{:else if errorMessage}
		<div
			class="mt-6 rounded-lg bg-zinc-950 p-4 text-sm text-zinc-200 ring-1 ring-red-950/60 ring-inset"
		>
			<div class="font-[var(--dorch-mono)] tracking-wide text-red-200">
				Failed to load your servers
			</div>
			<div class="mt-1 text-zinc-300">{errorMessage}</div>
			<button
				type="button"
				onclick={refresh}
				class="mt-4 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700"
			>
				Retry
			</button>
		</div>
	{:else}
		<div class="mt-5 overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset">
			<div class="overflow-x-auto">
				<table class="min-w-full border-collapse text-left">
					<thead class="bg-red-950/25">
						<tr class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-200">
							<th class="px-4 py-3">MAP</th>
							<th class="px-4 py-3">SERVER</th>
							<th class="px-4 py-3">STATUS</th>
							<th class="px-4 py-3">VISIBILITY</th>
							<th class="px-4 py-3">PLAYERS</th>
							<th class="px-4 py-3">KILLS</th>
							<th class="px-4 py-3">IWAD</th>
						</tr>
					</thead>
					<tbody class="divide-y divide-red-950/40">
						{#if rows().length === 0}
							<tr>
								<td class="px-4 py-5 text-sm text-zinc-400" colspan="7">No servers found.</td>
							</tr>
						{:else}
							{#each rows() as row (row.game.game_id)}
								<tr
									class="cursor-pointer hover:bg-zinc-900/35"
									role="link"
									tabindex="0"
									aria-label={`Open game ${row.game.info?.name ?? row.game.spec?.name ?? row.game.game_id}`}
									onclick={() => openGame(row.game.game_id)}
									onkeydown={(e) => onRowKeyDown(e, row.game.game_id)}
								>
									<td class="px-4 py-3">
										<div class="flex items-center gap-3">
											{#if row.thumbnailUrl}
												<img
													src={row.thumbnailUrl}
													alt={row.game.info?.map_title ?? row.game.info?.current_map ?? 'Map'}
													class="h-10 w-16 shrink-0 rounded-md object-cover ring-1 ring-red-950/60 ring-inset"
													loading="lazy"
												/>
											{:else}
												<div
													class="h-10 w-16 shrink-0 rounded-md bg-zinc-900 ring-1 ring-red-950/60 ring-inset"
												></div>
											{/if}
											<div class="min-w-0">
												<div class="truncate text-sm font-semibold text-zinc-100">
													{row.game.info?.map_title ?? row.game.info?.current_map ?? 'UNKNOWN'}
												</div>
												<div class="truncate text-xs text-zinc-400">
													{#if row.game.files?.length}
														{row.pwadName}
													{:else}
														&nbsp;
													{/if}
												</div>
											</div>
										</div>
									</td>

									<td class="px-4 py-3">
										<div
											class="truncate text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100"
										>
											{row.game.info?.name ?? row.game.spec?.name ?? '(loading...)'}
										</div>
										<div class="mt-0.5 truncate text-xs text-zinc-400">{row.game.game_id}</div>
									</td>

									<td class="px-4 py-3">
										<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
											{row.game.status ?? 'Unknown'}
										</div>
									</td>

									<td class="px-4 py-3">
										<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
											{visibilityLabel(row.game.info?.private ?? row.game.spec?.private)}
										</div>
									</td>

									<td class="px-4 py-3">
										<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
											{row.game.info?.player_count ?? 0} / {row.game.info?.max_players ??
												row.game.spec?.max_players ??
												0}
										</div>
									</td>

									<td class="px-4 py-3">
										<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
											{#if row.game.info}
												{row.game.info.monster_kill_count} / {row.game.info.monster_count}
											{:else}
												&nbsp;
											{/if}
										</div>
									</td>

									<td class="px-4 py-3">
										<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
											{row.iwadName}
										</div>
									</td>
								</tr>
							{/each}
						{/if}
					</tbody>
				</table>
			</div>
		</div>
	{/if}
</section>
