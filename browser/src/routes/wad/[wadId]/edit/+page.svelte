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
	let notAuthenticated = $state(false);
	let authChecking = $state(false);
	let currentUserId = $state<string | null>(null);

	// Form fields
	let title = $state('');
	let description = $state('');
	let authors = $state('');

	// Track initial state for change detection
	let initialTitle = $state('');
	let initialDescription = $state('');
	let initialAuthors = $state('');
	let hasUnsavedChanges = $derived(
		title !== initialTitle || description !== initialDescription || authors !== initialAuthors
	);

	$effect(() => {
		notAuthenticated = data.notAuthenticated;
		authChecking = data.notAuthenticated;
		if (!data.notAuthenticated) authChecking = false;
	});

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
					Authorization: `Bearer ${token}`
				},
				body: JSON.stringify({
					title: title.trim() || null,
					description: description.trim() || null,
					authors: authors.trim()
						? authors
								.split(',')
								.map((a: string) => a.trim())
								.filter((a: string) => a.length > 0)
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

	// Delete flow
	let showDeleteModal = $state(false);
	let deleting = $state(false);

	async function handleDelete() {
		const token = await getAccessToken();
		if (!token) {
			showToast('You must be logged in to delete a WAD');
			return;
		}

		deleting = true;
		try {
			const res = await fetch(`https://api.gib.gg/wad/${data.wadId}`, {
				method: 'DELETE',
				headers: { Authorization: `Bearer ${token}` }
			});

			if (!res.ok) {
				if (res.status === 401) {
					showToast('Session expired. Please log in again.');
					return;
				}
				if (res.status === 403 || res.status === 404) {
					showToast('You are not authorized to delete this WAD.');
					return;
				}
				throw new Error(`Failed to delete: ${res.status}`);
			}

			showToast('WAD deleted');
			goto('/my-wads');
		} catch (err) {
			console.error('Failed to delete WAD:', err);
			showToast('Failed to delete WAD. Please try again.');
		} finally {
			deleting = false;
			showDeleteModal = false;
		}
	}
</script>

<svelte:head>
	<title>Edit WAD - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-4xl px-4 py-6">
	<div class="mb-8 flex items-center justify-between">
		<h1 class="text-3xl font-semibold tracking-tight">Edit WAD</h1>
		<a
			href={resolve(`/wad/${data.wadId}`)}
			class="text-sm text-zinc-400 transition-colors hover:text-zinc-200"
		>
			← Back to WAD
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
			<p class="mb-4 text-zinc-400">Please log in to edit WADs.</p>
			<a
				href="/#login"
				class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70"
			>
				Log In
			</a>
		</div>
	{:else if !data.wad}
		<div class="rounded-lg border border-red-900/50 bg-red-900/20 p-6 text-center">
			<p class="text-red-400">WAD not found.</p>
			<a
				href={resolve('/my-wads')}
				class="mt-4 inline-block rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700"
			>
				Back to My WADs
			</a>
		</div>
	{:else if !isOwner()}
		<div class="rounded-lg border border-red-900/50 bg-red-900/20 p-6 text-center">
			<p class="mb-2 font-semibold text-red-400">Not Authorized</p>
			<p class="mb-4 text-zinc-400">You can only edit WADs that you uploaded.</p>
			<a
				href={resolve(`/wad/${data.wadId}`)}
				class="inline-block rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700"
			>
				Back to WAD
			</a>
		</div>
	{:else}
		<div class="space-y-6">
			<!-- WAD Info -->
			<div class="rounded-lg bg-zinc-900/60 p-6 ring-1 ring-zinc-800">
				<div class="flex items-center gap-3">
					<svg
						class="h-8 w-8 text-zinc-400"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
						/>
					</svg>
					<div>
						<p class="font-medium text-zinc-100">{wadLabel(data.wad.meta)}</p>
						<p class="text-sm text-zinc-400">Published WAD • File cannot be changed</p>
					</div>
				</div>
			</div>

			<!-- Edit Details Section -->
			<div class="rounded-lg bg-zinc-900/60 p-6 ring-1 ring-zinc-800">
				<h2 class="mb-4 text-lg font-semibold">Details</h2>

				<div class="space-y-4">
					<div>
						<label for="title" class="mb-1 block text-sm font-medium text-zinc-300">Title</label>
						<input
							id="title"
							type="text"
							bind:value={title}
							placeholder="My Awesome WAD"
							class="w-full rounded-lg bg-zinc-800 px-4 py-2 text-zinc-100 ring-1 ring-zinc-700 placeholder:text-zinc-500 focus:ring-2 focus:ring-red-500 focus:outline-none"
						/>
					</div>

					<div>
						<label for="authors" class="mb-1 block text-sm font-medium text-zinc-300"
							>Authors <span class="text-xs font-normal text-zinc-500"
								>(comma-separated list of authors)</span
							></label
						>
						<input
							id="authors"
							type="text"
							bind:value={authors}
							placeholder="Author1, Author2"
							class="w-full rounded-lg bg-zinc-800 px-4 py-2 text-zinc-100 ring-1 ring-zinc-700 placeholder:text-zinc-500 focus:ring-2 focus:ring-red-500 focus:outline-none"
						/>
					</div>

					<div>
						<label for="description" class="mb-1 block text-sm font-medium text-zinc-300"
							>Description</label
						>
						<textarea
							id="description"
							bind:value={description}
							rows="4"
							placeholder="Describe your WAD..."
							class="w-full resize-none rounded-lg bg-zinc-800 px-4 py-2 text-zinc-100 ring-1 ring-zinc-700 placeholder:text-zinc-500 focus:ring-2 focus:ring-red-500 focus:outline-none"
						></textarea>
					</div>
				</div>
			</div>

			<!-- Action Buttons -->
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-3">
					<button
						type="button"
						onclick={() => (showDeleteModal = true)}
						disabled={saving || deleting}
						class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-red-400 ring-1 ring-red-800/50 transition-colors hover:bg-red-800/70 hover:text-red-300 disabled:cursor-not-allowed disabled:opacity-50"
					>
						Delete WAD
					</button>
					{#if hasUnsavedChanges}
						<span class="text-sm text-yellow-400">Unsaved changes</span>
					{/if}
				</div>

				<div class="flex items-center gap-3">
					<button
						type="button"
						onclick={handleCancel}
						disabled={saving}
						class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
					>
						Cancel
					</button>

					<button
						type="button"
						onclick={handleSave}
						disabled={!hasUnsavedChanges || saving}
						class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-6 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70 disabled:cursor-not-allowed disabled:opacity-50"
					>
						{#if saving}
							<div
								class="h-4 w-4 animate-spin rounded-full border-2 border-red-300 border-t-white"
							></div>
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

{#if showDeleteModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
		<button
			type="button"
			class="absolute inset-0 bg-zinc-950/80"
			onclick={() => {
				if (!deleting) showDeleteModal = false;
			}}
			aria-label="Close dialog"
		></button>
		<div
			class="relative w-full max-w-md rounded-xl bg-zinc-900 p-6 ring-1 ring-zinc-700 ring-inset"
			role="dialog"
			aria-modal="true"
		>
			<h2 class="text-lg font-semibold text-zinc-100">Are you sure?</h2>
			<p class="mt-2 text-sm text-zinc-400">This cannot be undone.</p>
			<div class="mt-6 flex items-center justify-end gap-3">
				<button
					type="button"
					onclick={() => (showDeleteModal = false)}
					disabled={deleting}
					class="rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
				>
					Cancel
				</button>
				<button
					type="button"
					onclick={handleDelete}
					disabled={deleting}
					class="rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70 disabled:cursor-not-allowed disabled:opacity-50"
				>
					{#if deleting}
						<div
							class="mr-2 inline-block h-4 w-4 animate-spin rounded-full border-2 border-red-300 border-t-white"
						></div>
						Deleting...
					{:else}
						Delete
					{/if}
				</button>
			</div>
		</div>
	</div>
{/if}
