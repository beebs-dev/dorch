<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { browser } from '$app/environment';
	import { getAccessToken, subscribe as authSubscribe } from '$lib/stores/auth';
	import { showToast } from '$lib/stores/toast';
	import type { WadDraft, UpdateDraftRequest, UploadResponse } from '$lib/types/wadinfo';
	import { humanBytes } from '$lib/utils/format';

	const apiBaseUrl = 'https://api.gib.gg';
	const supportedExtensions = ['.wad', '.pk3', '.wad.gz', '.pk3.gz'];
	const maxFileSize = 1000 * 1024 * 1024; // 1000 MiB

	let { data } = $props();

	// State
	let saving = $state(false);
	let uploading = $state(false);
	let publishing = $state(false);
	let notAuthenticated = $state(data.notAuthenticated);
	let error = $state<string | null>(data.loadError ?? null);

	// Draft data - initialize from SSR
	let draft = $state<WadDraft | null>(data.draft);
	let title = $state(data.draft?.title ?? '');
	let author = $state(data.draft?.author ?? '');
	let description = $state(data.draft?.description ?? '');
	let aiEnabled = $state(data.draft?.ai_enabled ?? true);
	let uploadedFile = $state<{ name: string; size: number; hash: string } | null>(
		data.draft?.upload_id && data.draft?.file_size && data.draft?.file_sha256
			? { name: 'Previously uploaded file', size: data.draft.file_size, hash: data.draft.file_sha256 }
			: null
	);

	// Track unsaved changes
	let hasUnsavedChanges = $state(false);
	let initialState = $state<{ title: string; author: string; description: string; aiEnabled: boolean } | null>(
		data.draft ? { title: data.draft.title ?? '', author: data.draft.author ?? '', description: data.draft.description ?? '', aiEnabled: data.draft.ai_enabled } : null
	);

	// Compute unsaved changes by comparing current state to initial
	$effect(() => {
		if (initialState) {
			hasUnsavedChanges =
				title !== initialState.title ||
				author !== initialState.author ||
				description !== initialState.description ||
				aiEnabled !== initialState.aiEnabled;
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

	async function saveDraft() {
		if (!draft) return;
		if (saving) return;

		const token = await getValidAccessToken();
		if (!token) return;

		saving = true;

		try {
			const updateReq: UpdateDraftRequest = {
				title: title || null,
				author: author || null,
				description: description || null,
				ai_enabled: aiEnabled
			};

			const res = await fetch(`${apiBaseUrl}/draft/${draft.draft_id}`, {
				method: 'PUT',
				headers: {
					authorization: `Bearer ${token}`,
					'content-type': 'application/json',
					accept: 'application/json'
				},
				body: JSON.stringify(updateReq)
			});

			if (!res.ok) {
				throw new Error(`Failed to save draft: ${res.status}`);
			}

			const updated: WadDraft = await res.json();
			draft = updated;

			// Update initial state after successful save
			initialState = { title, author, description, aiEnabled };
			hasUnsavedChanges = false;

			showToast('Draft saved');
		} catch (e) {
			showToast('Failed to save draft');
		} finally {
			saving = false;
		}
	}

	async function handleFileSelect(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		// Validate file extension
		const lowerName = file.name.toLowerCase();
		const isValidExtension = supportedExtensions.some((ext) => lowerName.endsWith(ext));
		if (!isValidExtension) {
			showToast(`Unsupported file type. Supported: ${supportedExtensions.join(', ')}`);
			input.value = '';
			return;
		}

		// Validate file size
		if (file.size > maxFileSize) {
			showToast(`File too large. Maximum size: ${humanBytes(maxFileSize)}`);
			input.value = '';
			return;
		}

		await uploadFile(file);
		input.value = '';
	}

	async function uploadFile(file: File) {
		if (!draft) return;

		const token = await getValidAccessToken();
		if (!token) return;

		uploading = true;

		try {
			// Read file as ArrayBuffer and compute hash locally
			const arrayBuffer = await file.arrayBuffer();
			const hashBuffer = await crypto.subtle.digest('SHA-256', arrayBuffer);
			const hashArray = Array.from(new Uint8Array(hashBuffer));
			const localHash = hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');

			// Upload to server
			const formData = new FormData();
			formData.append('file', file);

			const uploadRes = await fetch(`${apiBaseUrl}/upload`, {
				method: 'POST',
				headers: {
					authorization: `Bearer ${token}`
				},
				body: formData
			});

			if (!uploadRes.ok) {
				const text = await uploadRes.text();
				throw new Error(`Upload failed: ${uploadRes.status} - ${text}`);
			}

			const uploadData: UploadResponse = await uploadRes.json();

			// Verify hash matches
			if (uploadData.hash !== localHash) {
				showToast('File verification failed. Hash mismatch.');
				return;
			}

			// Update draft with upload info
			const updateReq: UpdateDraftRequest = {
				upload_id: uploadData.id,
				file_sha256: uploadData.hash,
				file_size: uploadData.size
			};

			const updateRes = await fetch(`${apiBaseUrl}/draft/${draft.draft_id}`, {
				method: 'PUT',
				headers: {
					authorization: `Bearer ${token}`,
					'content-type': 'application/json',
					accept: 'application/json'
				},
				body: JSON.stringify(updateReq)
			});

			if (!updateRes.ok) {
				throw new Error(`Failed to update draft with upload: ${updateRes.status}`);
			}

			const updated: WadDraft = await updateRes.json();
			draft = updated;

			uploadedFile = {
				name: file.name,
				size: uploadData.size,
				hash: uploadData.hash
			};

			showToast('File uploaded successfully');
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Upload failed';
			showToast(message);
		} finally {
			uploading = false;
		}
	}

	async function publishDraft() {
		if (!draft) return;
		if (!draft.upload_id) {
			showToast('Please upload a file before publishing');
			return;
		}

		// Save any pending changes first
		if (hasUnsavedChanges) {
			await saveDraft();
		}

		const token = await getValidAccessToken();
		if (!token) return;

		publishing = true;

		try {
			const res = await fetch(`${apiBaseUrl}/draft/${draft.draft_id}/publish`, {
				method: 'POST',
				headers: {
					authorization: `Bearer ${token}`,
					accept: 'application/json'
				}
			});

			if (!res.ok) {
				const text = await res.text();
				throw new Error(`Publish failed: ${res.status} - ${text}`);
			}

			showToast('WAD published successfully!');
			await goto(resolve('/my-wads'));
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Publish failed';
			showToast(message);
		} finally {
			publishing = false;
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

	// Warn before leaving with unsaved changes
	$effect(() => {
		if (!browser) return;
		const handleBeforeUnload = (e: BeforeUnloadEvent) => {
			if (hasUnsavedChanges) {
				e.preventDefault();
				e.returnValue = '';
			}
		};
		window.addEventListener('beforeunload', handleBeforeUnload);
		return () => {
			window.removeEventListener('beforeunload', handleBeforeUnload);
		};
	});

	const canPublish = $derived(draft?.upload_id != null);
	const isPublished = $derived(draft?.status === 'published');
</script>

<svelte:head>
	<title>{isPublished ? 'EDIT WAD' : 'UPLOAD WAD'} - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-4xl px-4 py-6">
	<div class="flex items-center justify-between mb-8">
		<h1 class="text-3xl font-semibold tracking-tight">
			{isPublished ? 'Edit WAD' : 'Upload WAD'}
		</h1>
		<a
			href={resolve('/my-wads')}
			class="text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
		>
			← Back to My WADs
		</a>
	</div>

	{#if notAuthenticated}
		<div class="rounded-lg bg-zinc-900/50 p-8 text-center">
			<p class="text-zinc-400 mb-4">Please log in to upload WADs.</p>
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
			<a
				href={resolve('/upload')}
				class="mt-4 inline-block rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors"
			>
				Retry
			</a>
		</div>
	{:else if draft}
		<div class="space-y-6">
			<!-- File Upload Section -->
			<div class="rounded-lg bg-zinc-900/60 ring-1 ring-zinc-800 p-6">
				<h2 class="text-lg font-semibold mb-4">WAD File</h2>

				{#if uploadedFile}
					<div class="flex items-center justify-between bg-zinc-800/50 rounded-lg p-4">
						<div class="flex items-center gap-3">
							<svg class="h-8 w-8 text-green-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
							</svg>
							<div>
								<p class="font-medium text-zinc-100">{uploadedFile.name}</p>
								<p class="text-sm text-zinc-400">{humanBytes(uploadedFile.size)}</p>
							</div>
						</div>
						<label class="cursor-pointer rounded-lg bg-zinc-700 px-3 py-1.5 text-sm font-semibold text-zinc-200 hover:bg-zinc-600 transition-colors">
							Replace
							<input
								type="file"
								accept=".wad,.pk3,.gz"
								class="hidden"
								onchange={handleFileSelect}
								disabled={uploading}
							/>
						</label>
					</div>
				{:else}
					<label class="flex flex-col items-center justify-center w-full h-40 border-2 border-dashed border-zinc-700 rounded-lg cursor-pointer hover:border-zinc-500 transition-colors">
						{#if uploading}
							<div class="flex flex-col items-center">
								<div class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-red-500 mb-2"></div>
								<p class="text-sm text-zinc-400">Uploading...</p>
							</div>
						{:else}
							<div class="flex flex-col items-center">
								<svg class="h-10 w-10 text-zinc-500 mb-2" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path stroke-linecap="round" stroke-linejoin="round" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
								</svg>
								<p class="text-sm text-zinc-400 mb-1">Click to upload or drag and drop</p>
								<p class="text-xs text-zinc-500">Supported: {supportedExtensions.join(', ')}</p>
								<p class="text-xs text-zinc-500">Max size: {humanBytes(maxFileSize)}</p>
							</div>
						{/if}
						<input
							type="file"
							accept=".wad,.pk3,.gz"
							class="hidden"
							onchange={handleFileSelect}
							disabled={uploading}
						/>
					</label>
				{/if}
			</div>

			<!-- Metadata Section -->
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
						<label for="author" class="block text-sm font-medium text-zinc-300 mb-1">Author</label>
						<input
							id="author"
							type="text"
							bind:value={author}
							placeholder="Your name or handle"
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

					<div class="flex items-center gap-3">
						<input
							id="ai-enabled"
							type="checkbox"
							bind:checked={aiEnabled}
							class="h-4 w-4 rounded border-zinc-700 bg-zinc-800 text-red-500 focus:ring-red-500 focus:ring-offset-zinc-900"
						/>
						<label for="ai-enabled" class="text-sm text-zinc-300">
							Enable AI Analysis
							<span class="text-zinc-500 ml-1">(generates descriptions and tags automatically)</span>
						</label>
					</div>
				</div>
			</div>

			<!-- Action Buttons -->
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-3">
					<button
						type="button"
						onclick={saveDraft}
						disabled={!hasUnsavedChanges || saving}
						class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 hover:bg-zinc-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{#if saving}
							<div class="h-4 w-4 animate-spin rounded-full border-2 border-zinc-600 border-t-zinc-200"></div>
							Saving...
						{:else}
							Save Draft
						{/if}
					</button>

					{#if hasUnsavedChanges}
						<span class="text-sm text-yellow-400">Unsaved changes</span>
					{/if}
				</div>

				{#if !isPublished}
					<button
						type="button"
						onclick={publishDraft}
						disabled={!canPublish || publishing}
						class="inline-flex items-center gap-2 rounded-lg bg-red-700 px-6 py-2 text-sm font-semibold text-white hover:bg-red-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
					>
						{#if publishing}
							<div class="h-4 w-4 animate-spin rounded-full border-2 border-red-300 border-t-white"></div>
							Publishing...
						{:else}
							Publish WAD
						{/if}
					</button>
				{:else}
					<span class="px-3 py-1.5 rounded text-sm font-medium bg-green-900/50 text-green-300">
						✓ Published
					</span>
				{/if}
			</div>

			{#if !canPublish && !isPublished}
				<p class="text-sm text-zinc-500 text-center">
					Upload a file to enable publishing.
				</p>
			{/if}
		</div>
	{/if}
</section>
