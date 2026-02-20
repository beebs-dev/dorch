<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { getAccessToken, subscribe as authSubscribe } from '$lib/stores/auth';
	import { showToast } from '$lib/stores/toast';
	import type { WadDraft, UserWad } from '$lib/types/wadinfo';
	import { humanBytes } from '$lib/utils/format';
	import { onMount } from 'svelte';

	const apiBaseUrl = 'https://api.gib.gg';

	let { data } = $props();

	let drafts = $state<WadDraft[]>(data.drafts);
	let publishedWads = $state<UserWad[]>(data.publishedWads);
	let error = $state<string | null>(data.loadError ?? null);
	let notAuthenticated = $state(data.notAuthenticated);
	let deleting = $state(false);
	let authChecking = $state(data.notAuthenticated); // True if we need to check for token refresh

	// Sync with SSR data changes (e.g., after invalidateAll)
	$effect(() => {
		drafts = data.drafts;
		publishedWads = data.publishedWads;
		error = data.loadError ?? null;
		notAuthenticated = data.notAuthenticated;
		// If we successfully loaded data, we're no longer checking
		if (!data.notAuthenticated) {
			authChecking = false;
		}
	});

	async function getValidAccessToken(): Promise<string | null> {
		const token = await getAccessToken();
		if (!token) {
			notAuthenticated = true;
			return null;
		}
		return token;
	}

	async function deleteDraft(draftId: string) {
		if (!confirm('Are you sure you want to delete this draft?')) {
			return;
		}

		const token = await getValidAccessToken();
		if (!token) return;

		deleting = true;
		try {
			const res = await fetch(`${apiBaseUrl}/draft/${draftId}`, {
				method: 'DELETE',
				headers: {
					authorization: `Bearer ${token}`
				}
			});

			if (!res.ok) {
				throw new Error(`Failed to delete draft: ${res.status}`);
			}

			showToast('Draft deleted');
			// Refresh data from server
			await invalidateAll();
		} catch (e) {
			showToast('Failed to delete draft');
		} finally {
			deleting = false;
		}
	}

	onMount(() => {
		// If SSR returned notAuthenticated, try to refresh the token
		// (the access token cookie may have expired but refresh token in localStorage is still valid)
		if (data.notAuthenticated) {
			getAccessToken().then((token) => {
				if (token) {
					// Token refresh succeeded, reload the page data
					invalidateAll().then(() => {
						authChecking = false;
						notAuthenticated = false;
					});
				} else {
					// No valid token available
					authChecking = false;
				}
			}).catch(() => {
				authChecking = false;
			});
		}

		const unsubscribeAuth = authSubscribe((state) => {
			// Don't update notAuthenticated while we're still checking for a valid refresh token
			if (authChecking) return;
			if (!state.isAuthenticated) {
				notAuthenticated = true;
			}
		});

		return () => {
			unsubscribeAuth();
		};
	});

	function formatDate(timestamp: number): string {
		return new Date(timestamp).toLocaleDateString('en-US', {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	// Filter drafts to only show actual drafts (not published ones that might still be in the drafts table)
	const draftItems = $derived(drafts.filter((d) => d.status === 'draft'));
</script>

<svelte:head>
	<title>MANAGE WADS - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<div class="flex items-center justify-between mb-8">
		<h1 class="text-3xl font-semibold tracking-tight">Manage WADs</h1>
		<a
			href={resolve('/upload')}
			class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-4 py-2 text-sm font-semibold text-white hover:bg-red-600 transition-colors"
		>
			<svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
				<path d="M10 3a1 1 0 011 1v5h5a1 1 0 110 2h-5v5a1 1 0 11-2 0v-5H4a1 1 0 110-2h5V4a1 1 0 011-1z" />
			</svg>
			Upload WAD
		</a>
	</div>

	{#if authChecking}
		<div class="flex items-center justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-red-500"></div>
		</div>
	{:else if notAuthenticated}
		<div class="rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="text-zinc-400 mb-4">Please log in to manage your WADs.</p>
			<a
				href="/#login"
				class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-4 py-2 text-sm font-semibold text-white hover:bg-red-600 transition-colors"
			>
				Log In
			</a>
		</div>
	{:else if error}
		<div class="rounded-lg bg-red-900/20 border border-red-900/50 p-6 text-center">
			<p class="text-red-400">{error}</p>
			<button
				type="button"
				onclick={() => invalidateAll()}
				class="mt-4 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors"
			>
				Retry
			</button>
		</div>
	{:else if drafts.length === 0 && publishedWads.length === 0}
		<div class="rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="text-zinc-400 mb-4">You haven't uploaded any WADs yet.</p>
			<a
				href={resolve('/upload')}
				class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-4 py-2 text-sm font-semibold text-white hover:bg-red-600 transition-colors"
			>
				Upload Your First WAD
			</a>
		</div>
	{:else}
		{#if draftItems.length > 0}
			<section class="mb-8">
				<h2 class="text-xl font-semibold mb-4 text-zinc-300">Drafts</h2>
				<div class="space-y-3">
					{#each draftItems as draft (draft.draft_id)}
						<div class="flex items-center justify-between rounded-lg bg-zinc-900/60 ring-1 ring-zinc-800 p-4">
							<div class="flex-1 min-w-0">
								<h3 class="font-medium text-zinc-100 truncate">
									{draft.title || 'Untitled Draft'}
								</h3>
								<div class="flex items-center gap-4 mt-1 text-sm text-zinc-400">
									{#if draft.author}
										<span>by {draft.author}</span>
									{/if}
									{#if draft.file_size}
										<span>{humanBytes(draft.file_size)}</span>
									{/if}
									<span>Updated {formatDate(draft.updated_at)}</span>
								</div>
							</div>
							<div class="flex items-center gap-2 ml-4">
								<span class="px-2 py-1 rounded text-xs font-medium bg-yellow-900/50 text-yellow-300">
									Draft
								</span>
								<a
									href={`/upload?draft=${draft.draft_id}`}
									class="inline-flex items-center gap-1 rounded-lg bg-zinc-800 px-3 py-1.5 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors"
								>
									Edit
								</a>
								<button
									type="button"
									onclick={() => deleteDraft(draft.draft_id)}
									class="inline-flex items-center gap-1 rounded-lg bg-red-900/50 px-3 py-1.5 text-sm font-semibold text-red-300 hover:bg-red-900/70 transition-colors"
								>
									Delete
								</button>
							</div>
						</div>
					{/each}
				</div>
			</section>
		{/if}

		{#if publishedWads.length > 0}
			<section>
				<h2 class="text-xl font-semibold mb-4 text-zinc-300">Published WADs</h2>
				<div class="space-y-3">
					{#each publishedWads as wad (wad.wad_id)}
						<a
							href={resolve(`/wad/${wad.wad_id}`)}
							class="flex items-center justify-between rounded-lg bg-zinc-900/60 ring-1 ring-zinc-800 p-4 hover:bg-zinc-800/60 transition-colors cursor-pointer"
						>
							<div class="flex-1 min-w-0">
								<h3 class="font-medium text-zinc-100 truncate">
									{wad.title || wad.preferred_filename || 'Untitled WAD'}
								</h3>
								<div class="flex items-center gap-4 mt-1 text-sm text-zinc-400">
									{#if wad.file_size_bytes}
										<span>{humanBytes(wad.file_size_bytes)}</span>
									{/if}
									<span>Published {formatDate(wad.updated_at)}</span>
								</div>
							</div>
							<div class="flex items-center gap-2 ml-4">
								<span class="px-2 py-1 rounded text-xs font-medium bg-green-900/50 text-green-300">
									Published
								</span>
							</div>
						</a>
					{/each}
				</div>
			</section>
		{/if}
	{/if}
</section>
