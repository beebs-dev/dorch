<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { getAccessToken, subscribe as authSubscribe } from '$lib/stores/auth';
	import { showToast } from '$lib/stores/toast';
	import { wadLabel } from '$lib/utils/format';

	let { data } = $props();

	// State
	let saving = $state(false);
	let notAuthenticated = $state(data.notAuthenticated);
	let authChecking = $state(data.notAuthenticated);
	let currentUserId = $state<string | null>(null);

	// Form fields
	let title = $state(data.wad?.meta?.title ?? '');
	let description = $state(data.wad?.description ?? '');
	let authors = $state((data.wad?.meta?.authors ?? []).join(', '));

	// Track initial state for change detection
	let initialTitle = $state(data.wad?.meta?.title ?? '');
	let initialDescription = $state(data.wad?.description ?? '');
	let initialAuthors = $state((data.wad?.meta?.authors ?? []).join(', '));
	let hasUnsavedChanges = $derived(title !== initialTitle || description !== initialDescription || authors !== initialAuthors);

	// Re-sync form fields when data changes (e.g. after auth-triggered reload)
	$effect(() => {
		const wad = data.wad;
		const t = wad?.meta?.title ?? '';
		const d = wad?.description ?? '';
		const a = (wad?.meta?.authors ?? []).join(', ');
		title = t;
		description = d;
		authors = a;
		initialTitle = t;
		initialDescription = d;
		initialAuthors = a;
	});

	// Authorization check
	const isOwner = $derived(() => {
		if (!currentUserId || !data.wad?.uploader_id) return false;
		return currentUserId === data.wad.uploader_id;
	});

	onMount(() => {
		const unsubscribeAuth = authSubscribe((state) => {
			currentUserId = state.userId;
			
			// If we had notAuthenticated from SSR but have a token now, refetch
			if (notAuthenticated && state.isAuthenticated) {
				authChecking = false;
				notAuthenticated = false;
				// Reload to get fresh data with token
				goto(window.location.href, { replaceState: true, invalidateAll: true });
			} else {
				authChecking = false;
			}
		});
		return () => unsubscribeAuth();
	});

	async function handleSave() {
		const token = await getAccessToken();
		if (!token) {
			showToast('You must be logged in to save changes');
			return;
		}

		saving = true;
		try {
			const res = await fetch(`https://api.gib.gg/wad/${data.wadId}`, {
				method: 'PUT',
				headers: {
					'Content-Type': 'application/json',
					'Authorization': `Bearer ${token}`
				},
				body: JSON.stringify({
					title: title.trim() || null,
					description: description.trim() || null,
					authors: authors.trim()
						? authors.split(',').map((a: string) => a.trim()).filter((a: string) => a.length > 0)
						: null
				})
			});

			if (!res.ok) {
				if (res.status === 401) {
					showToast('Session expired. Please log in again.');
					return;
				}
				if (res.status === 403) {
					showToast('You are not authorized to edit this WAD.');
					return;
				}
				throw new Error(`Failed to save: ${res.status}`);
			}

			showToast('WAD updated successfully');
			initialTitle = title;
			initialDescription = description;
			initialAuthors = authors;
			goto(`/wad/${data.wadId}`);
		} catch (err) {
			console.error('Failed to save WAD:', err);
			showToast('Failed to save changes. Please try again.');
		} finally {
			saving = false;
		}
	}

	function handleCancel() {
		goto(`/wad/${data.wadId}`);
	}
</script>

<svelte:head>
	<title>Edit WAD - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-4xl px-4 py-6">
	<div class="flex items-center justify-between mb-8">
		<h1 class="text-3xl font-semibold tracking-tight">Edit WAD</h1>
		<a
			href={resolve(`/wad/${data.wadId}`)}
			class="text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
		>
			← Back to WAD
		</a>
	</div>

	{#if authChecking}
		<div class="flex items-center justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-red-500"></div>
		</div>
	{:else if notAuthenticated}
		<div class="rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="text-zinc-400 mb-4">Please log in to edit WADs.</p>
			<a
				href="/#login"
				class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-4 py-2 text-sm font-semibold text-white hover:bg-red-600 transition-colors"
			>
				Log In
			</a>
		</div>
	{:else if !data.wad}
		<div class="rounded-lg bg-red-900/20 border border-red-900/50 p-6 text-center">
			<p class="text-red-400">WAD not found.</p>
			<a
				href={resolve('/my-wads')}
				class="mt-4 inline-block rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors"
			>
				Back to My WADs
			</a>
		</div>
	{:else if !isOwner()}
		<div class="rounded-lg bg-red-900/20 border border-red-900/50 p-6 text-center">
			<p class="text-red-400 mb-2 font-semibold">Not Authorized</p>
			<p class="text-zinc-400 mb-4">You can only edit WADs that you uploaded.</p>
			<a
				href={resolve(`/wad/${data.wadId}`)}
				class="inline-block rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors"
			>
				Back to WAD
			</a>
		</div>
	{:else}
		<div class="space-y-6">
			<!-- WAD Info -->
			<div class="rounded-lg bg-zinc-900/60 ring-1 ring-zinc-800 p-6">
				<div class="flex items-center gap-3">
					<svg class="h-8 w-8 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
					</svg>
					<div>
						<p class="font-medium text-zinc-100">{wadLabel(data.wad.meta)}</p>
						<p class="text-sm text-zinc-400">Published WAD • File cannot be changed</p>
					</div>
				</div>
			</div>

			<!-- Edit Details Section -->
			<div class="rounded-lg bg-zinc-900/60 ring-1 ring-zinc-800 p-6">
				<h2 class="text-lg font-semibold mb-4">Details</h2>

				<div class="space-y-4">
					<div>
						<label for="title" class="block text-sm font-medium text-zinc-300 mb-1">Title</label>
						<input
							id="title"
							type="text"
							bind:value={title}
							placeholder="My Awesome WAD"
							class="w-full rounded-lg bg-zinc-800 px-4 py-2 text-zinc-100 ring-1 ring-zinc-700 placeholder:text-zinc-500 focus:ring-2 focus:ring-red-500 focus:outline-none"
						/>
					</div>

					<div>
						<label for="authors" class="block text-sm font-medium text-zinc-300 mb-1">Authors <span class="text-xs text-zinc-500 font-normal">(comma-separated list of authors)</span></label>
						<input
							id="authors"
							type="text"
							bind:value={authors}
							placeholder="Author1, Author2"
							class="w-full rounded-lg bg-zinc-800 px-4 py-2 text-zinc-100 ring-1 ring-zinc-700 placeholder:text-zinc-500 focus:ring-2 focus:ring-red-500 focus:outline-none"
						/>
					</div>

					<div>
						<label for="description" class="block text-sm font-medium text-zinc-300 mb-1">Description</label>
						<textarea
							id="description"
							bind:value={description}
							rows="4"
							placeholder="Describe your WAD..."
							class="w-full rounded-lg bg-zinc-800 px-4 py-2 text-zinc-100 ring-1 ring-zinc-700 placeholder:text-zinc-500 focus:ring-2 focus:ring-red-500 focus:outline-none resize-none"
						></textarea>
					</div>
				</div>
			</div>

			<!-- Action Buttons -->
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-3">
					{#if hasUnsavedChanges}
						<span class="text-sm text-yellow-400">Unsaved changes</span>
					{/if}
				</div>

				<div class="flex items-center gap-3">
					<button
						type="button"
						onclick={handleCancel}
						disabled={saving}
						class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						Cancel
					</button>

					<button
						type="button"
						onclick={handleSave}
						disabled={!hasUnsavedChanges || saving}
						class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-6 py-2 text-sm font-semibold text-white hover:bg-red-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{#if saving}
							<div class="h-4 w-4 animate-spin rounded-full border-2 border-red-300 border-t-white"></div>
							Saving...
						{:else}
							Save Changes
						{/if}
					</button>
				</div>
			</div>
		</div>
	{/if}
</section>
