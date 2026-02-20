<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { getAccessToken, subscribe as authSubscribe } from '$lib/stores/auth';
	import { showToast } from '$lib/stores/toast';
	import type { WadDraft } from '$lib/types/wadinfo';
	import { humanBytes } from '$lib/utils/format';
	import { onMount } from 'svelte';

	const apiBaseUrl = 'https://api.gib.gg';

	let { data } = $props();

	let drafts = $state<WadDraft[]>(data.drafts);
	let error = $state<string | null>(data.loadError ?? null);
	let notAuthenticated = $state(data.notAuthenticated);
	let deleting = $state(false);

	// Sync with SSR data changes (e.g., after invalidateAll)
	$effect(() => {
		drafts = data.drafts;
		error = data.loadError ?? null;
		notAuthenticated = data.notAuthenticated;
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
		const unsubscribeAuth = authSubscribe((state) => {
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

	// Separate drafts and published WADs
	const draftItems = $derived(drafts.filter((d) => d.status === 'draft'));
	const publishedItems = $derived(drafts.filter((d) => d.status === 'published'));
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

	{#if notAuthenticated}
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
	{:else if drafts.length === 0}
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

		{#if publishedItems.length > 0}
			<section>
				<h2 class="text-xl font-semibold mb-4 text-zinc-300">Published WADs</h2>
				<div class="space-y-3">
					{#each publishedItems as wad (wad.draft_id)}
						<a
							href={wad.wad_id ? resolve(`/wad/${wad.wad_id}`) : '#'}
							class="flex items-center justify-between rounded-lg bg-zinc-900/60 ring-1 ring-zinc-800 p-4 hover:bg-zinc-800/60 transition-colors cursor-pointer"
						>
							<div class="flex-1 min-w-0">
								<h3 class="font-medium text-zinc-100 truncate">
									{wad.title || 'Untitled WAD'}
								</h3>
								<div class="flex items-center gap-4 mt-1 text-sm text-zinc-400">
									{#if wad.author}
										<span>by {wad.author}</span>
									{/if}
									{#if wad.file_size}
										<span>{humanBytes(wad.file_size)}</span>
									{/if}
									<span>Published {formatDate(wad.updated_at)}</span>
								</div>
							</div>
							<div class="flex items-center gap-2 ml-4">
								<span class="px-2 py-1 rounded text-xs font-medium bg-green-900/50 text-green-300">
									Published
								</span>
								<button
									type="button"
									onclick={(e) => { e.preventDefault(); e.stopPropagation(); goto(resolve(`/upload?draft=${wad.draft_id}`)); }}
									class="inline-flex items-center gap-1 rounded-lg bg-zinc-800 px-3 py-1.5 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors"
								>
									Manage
								</button>
							</div>
						</a>
					{/each}
				</div>
			</section>
		{/if}
	{/if}
</section>
