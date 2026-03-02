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

	let drafts = $state<WadDraft[]>([]);
	let publishedWads = $state<UserWad[]>([]);
	let error = $state<string | null>(null);
	let notAuthenticated = $state(false);
	let deleting = $state(false);
	let authChecking = $state(false); // True if we need to check for token refresh

	// Sync with SSR data changes (e.g., after invalidateAll)
	$effect(() => {
		drafts = data.drafts;
		publishedWads = data.publishedWads;
		error = data.loadError ?? null;
		notAuthenticated = data.notAuthenticated;
		authChecking = data.notAuthenticated;
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
			getAccessToken()
				.then((token) => {
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
				})
				.catch(() => {
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
	<div class="mb-8 flex items-center justify-between">
		<h1 class="text-3xl font-semibold tracking-tight">Manage WADs</h1>
		<a
			href={resolve('/upload')}
			class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70"
		>
			<svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
				<path
					d="M10 3a1 1 0 011 1v5h5a1 1 0 110 2h-5v5a1 1 0 11-2 0v-5H4a1 1 0 110-2h5V4a1 1 0 011-1z"
				/>
			</svg>
			Upload WAD
		</a>
	</div>

	{#if authChecking}
		<div class="flex items-center justify-center py-12">
			<div
				class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-red-500"
			></div>
		</div>
	{:else if notAuthenticated}
		<div class="rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="mb-4 text-zinc-400">Please log in to manage your WADs.</p>
			<a
				href="/#login"
				class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70"
			>
				Log In
			</a>
		</div>
	{:else if error}
		<div class="rounded-lg border border-red-900/50 bg-red-900/20 p-6 text-center">
			<p class="text-red-400">{error}</p>
			<button
				type="button"
				onclick={() => invalidateAll()}
				class="mt-4 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700"
			>
				Retry
			</button>
		</div>
	{:else if drafts.length === 0 && publishedWads.length === 0}
		<div class="rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="mb-4 text-zinc-400">You haven't uploaded any WADs yet.</p>
			<a
				href={resolve('/upload')}
				class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70"
			>
				Upload Your First WAD
			</a>
		</div>
	{:else}
		{#if draftItems.length > 0}
			<section class="mb-8">
				<h2 class="mb-4 text-xl font-semibold text-zinc-300">Drafts</h2>
				<div class="space-y-3">
					{#each draftItems as draft (draft.draft_id)}
						<div
							class="flex items-center justify-between rounded-lg bg-zinc-900/60 p-4 ring-1 ring-zinc-800"
						>
							<div class="min-w-0 flex-1">
								<h3 class="truncate font-medium text-zinc-100">
									{draft.title || 'Untitled Draft'}
								</h3>
								<div class="mt-1 flex items-center gap-4 text-sm text-zinc-400">
									{#if draft.author}
										<span>by {draft.author}</span>
									{/if}
									{#if draft.file_size}
										<span>{humanBytes(draft.file_size)}</span>
									{/if}
									<span>Updated {formatDate(draft.updated_at)}</span>
								</div>
							</div>
							<div class="ml-4 flex items-center gap-2">
								<span
									class="rounded bg-yellow-900/50 px-2 py-1 text-xs font-medium text-yellow-300"
								>
									Draft
								</span>
								<a
									href={`/upload?draft=${draft.draft_id}`}
									class="inline-flex items-center gap-1 rounded-lg bg-zinc-800 px-3 py-1.5 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700"
								>
									Edit
								</a>
								<button
									type="button"
									onclick={() => deleteDraft(draft.draft_id)}
									class="inline-flex items-center gap-1 rounded-lg bg-red-900/70 px-3 py-1.5 text-sm font-semibold text-red-300 transition-colors hover:bg-red-800/70"
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
				<h2 class="mb-4 text-xl font-semibold text-zinc-300">Published WADs</h2>
				<div class="space-y-3">
					{#each publishedWads as wad (wad.wad_id)}
						<a
							href={resolve(`/wad/${wad.wad_id}`)}
							class="flex cursor-pointer items-center justify-between rounded-lg bg-zinc-900/60 p-4 ring-1 ring-zinc-800 transition-colors hover:bg-zinc-800/60"
						>
							<div class="min-w-0 flex-1">
								<h3 class="truncate font-medium text-zinc-100">
									{wad.title || wad.preferred_filename || 'Untitled WAD'}
								</h3>
								<div class="mt-1 flex items-center gap-4 text-sm text-zinc-400">
									{#if wad.file_size_bytes}
										<span>{humanBytes(wad.file_size_bytes)}</span>
									{/if}
									<span>Published {formatDate(wad.updated_at)}</span>
								</div>
							</div>
							<div class="ml-4 flex items-center gap-2">
								<span class="rounded bg-green-900/50 px-2 py-1 text-xs font-medium text-green-300">
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
