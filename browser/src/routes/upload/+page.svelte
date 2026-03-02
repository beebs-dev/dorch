<script lang="ts">
	import { onMount } from 'svelte';
	import { goto, invalidateAll } from '$app/navigation';
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
	let deleting = $state(false);

	// Upload progress tracking
	let uploadProgress = $state(0); // 0-100
	let uploadedBytes = $state(0);
	let totalBytes = $state(0);
	let uploadStartTime = $state(0);
	let uploadSpeed = $state(0); // bytes per second
	let notAuthenticated = $state(false);
	let authChecking = $state(false); // True if we need to check for token refresh
	let error = $state<string | null>(null);

	// Draft data - initialize from SSR
	let draft = $state<WadDraft | null>(null);
	let title = $state('');
	let author = $state('');
	let description = $state('');
	let aiEnabled = $state(true);
	let uploadedFile = $state<{ name: string; size: number; hash: string } | null>(null);

	// Track unsaved changes
	let hasUnsavedChanges = $state(false);
	let initialState = $state<{
		title: string;
		author: string;
		description: string;
		aiEnabled: boolean;
	}>({
		title: '',
		author: '',
		description: '',
		aiEnabled: true
	});

	// Debounce timer for auto-save (plain variable to avoid reactivity)
	let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;
	// Snapshot of last saved values (non-reactive to prevent effect loops)
	let lastSavedState: {
		title: string;
		author: string;
		description: string;
		aiEnabled: boolean;
	} | null = null;

	function draftSnapshot(d: WadDraft | null): {
		title: string;
		author: string;
		description: string;
		aiEnabled: boolean;
	} {
		return d
			? {
					title: d.title ?? '',
					author: d.author ?? '',
					description: d.description ?? '',
					aiEnabled: d.ai_enabled
				}
			: { title: '', author: '', description: '', aiEnabled: true };
	}

	function uploadedFileFromDraft(
		d: WadDraft | null
	): { name: string; size: number; hash: string } | null {
		if (!d) return null;
		if (!d.upload_id || !d.file_size || !d.file_sha256) return null;
		return {
			name: d.filename ?? 'Previously uploaded file',
			size: d.file_size,
			hash: d.file_sha256
		};
	}

	// Sync local state from SSR data (and after invalidateAll navigations)
	$effect(() => {
		notAuthenticated = data.notAuthenticated;
		authChecking = data.notAuthenticated;
		error = data.loadError ?? null;

		draft = data.draft;
		title = data.draft?.title ?? '';
		author = data.draft?.author ?? '';
		description = data.draft?.description ?? '';
		aiEnabled = data.draft?.ai_enabled ?? true;
		uploadedFile = uploadedFileFromDraft(data.draft);

		initialState = draftSnapshot(data.draft);
		lastSavedState = draftSnapshot(data.draft);
		if (!data.notAuthenticated) authChecking = false;
	});

	// Flag to prevent concurrent draft creation
	let creatingDraft = false;

	// Compute unsaved changes by comparing current state to initial
	$effect(() => {
		hasUnsavedChanges =
			title !== initialState.title ||
			author !== initialState.author ||
			description !== initialState.description ||
			aiEnabled !== initialState.aiEnabled;
	});

	// Auto-save with 300ms debounce when fields change
	$effect(() => {
		// Track the fields we care about
		const _title = title;
		const _author = author;
		const _description = description;
		const _aiEnabled = aiEnabled;

		// Only proceed if we have saved state to compare against
		if (!lastSavedState) return;

		// Check if there are actual changes compared to last save
		const changed =
			_title !== lastSavedState.title ||
			_author !== lastSavedState.author ||
			_description !== lastSavedState.description ||
			_aiEnabled !== lastSavedState.aiEnabled;

		if (!changed) return;

		// Clear previous timer
		if (autoSaveTimer) {
			clearTimeout(autoSaveTimer);
		}

		// Set new debounced save
		autoSaveTimer = setTimeout(() => {
			// Update lastSavedState before saving to prevent re-trigger
			lastSavedState = {
				title: _title,
				author: _author,
				description: _description,
				aiEnabled: _aiEnabled
			};
			saveDraft();
		}, 300);

		return () => {
			if (autoSaveTimer) {
				clearTimeout(autoSaveTimer);
			}
		};
	});

	// Format upload speed in appropriate units
	function formatSpeed(bytesPerSec: number): string {
		if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`;
		if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
		return `${(bytesPerSec / (1024 * 1024)).toFixed(2)} MB/s`;
	}

	// Format remaining time
	function formatEta(seconds: number): string {
		if (!isFinite(seconds) || seconds <= 0) return '--';
		if (seconds < 60) return `${Math.ceil(seconds)}s`;
		if (seconds < 3600) {
			const mins = Math.floor(seconds / 60);
			const secs = Math.ceil(seconds % 60);
			return `${mins}m ${secs}s`;
		}
		const hours = Math.floor(seconds / 3600);
		const mins = Math.floor((seconds % 3600) / 60);
		return `${hours}h ${mins}m`;
	}

	// Calculate estimated time remaining
	const estimatedTimeRemaining = $derived(() => {
		if (uploadSpeed <= 0 || uploadedBytes >= totalBytes) return 0;
		const remainingBytes = totalBytes - uploadedBytes;
		return remainingBytes / uploadSpeed;
	});

	async function getValidAccessToken(): Promise<string | null> {
		const token = await getAccessToken();
		if (!token) {
			notAuthenticated = true;
			return null;
		}
		return token;
	}

	/** Lazily create a draft on first interaction. Returns true if draft is available. */
	async function ensureDraft(): Promise<boolean> {
		if (draft) return true;
		if (creatingDraft) return false;

		const token = await getValidAccessToken();
		if (!token) return false;

		creatingDraft = true;
		try {
			const res = await fetch(`${apiBaseUrl}/draft/resume`, {
				headers: {
					authorization: `Bearer ${token}`,
					accept: 'application/json'
				}
			});
			if (!res.ok) {
				throw new Error(`Failed to create draft: ${res.status}`);
			}
			const newDraft: WadDraft = await res.json();
			draft = newDraft;
			return true;
		} catch (e) {
			showToast('Failed to create draft');
			return false;
		} finally {
			creatingDraft = false;
		}
	}

	async function saveDraft() {
		if (saving) return;

		// Don't save if no fields have been edited and no file has been uploaded
		if (!hasUnsavedChanges && !uploadedFile) return;

		// Lazily create a draft on first save
		if (!(await ensureDraft())) return;
		const currentDraft = draft;
		if (!currentDraft) return;

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

			const res = await fetch(`${apiBaseUrl}/draft/${currentDraft.draft_id}`, {
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
			lastSavedState = { title, author, description, aiEnabled };
			hasUnsavedChanges = false;

			showToast('Draft saved');
		} catch (e) {
			showToast('Failed to save draft');
		} finally {
			saving = false;
		}
	}

	async function deleteDraft() {
		if (!draft) return;
		if (deleting) return;

		if (!confirm('Are you sure you want to delete this draft? This cannot be undone.')) {
			return;
		}

		const token = await getValidAccessToken();
		if (!token) return;

		deleting = true;

		try {
			const endpoint =
				draft.status === 'published' && draft.wad_id
					? `${apiBaseUrl}/wad/${draft.wad_id}`
					: `${apiBaseUrl}/draft/${draft.draft_id}`;

			const res = await fetch(endpoint, {
				method: 'DELETE',
				headers: {
					authorization: `Bearer ${token}`
				}
			});

			if (!res.ok) {
				throw new Error(`Failed to delete: ${res.status}`);
			}

			showToast('Deleted successfully');
			goto(resolve('/my-wads'), { invalidateAll: true });
		} catch (e) {
			showToast('Failed to delete');
		} finally {
			deleting = false;
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
		// Lazily create a draft on first upload
		if (!(await ensureDraft())) return;

		const token = await getValidAccessToken();
		if (!token) return;

		uploading = true;
		uploadProgress = 0;
		uploadedBytes = 0;
		totalBytes = file.size;
		uploadStartTime = Date.now();
		uploadSpeed = 0;

		try {
			// Read file as ArrayBuffer and compute hash locally
			const arrayBuffer = await file.arrayBuffer();
			const hashBuffer = await crypto.subtle.digest('SHA-256', arrayBuffer);
			const hashArray = Array.from(new Uint8Array(hashBuffer));
			const localHash = hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');

			// Upload to server using XMLHttpRequest for progress tracking
			const formData = new FormData();
			formData.append('file', file);

			const uploadData = await new Promise<UploadResponse>((resolve, reject) => {
				const xhr = new XMLHttpRequest();

				xhr.upload.addEventListener('progress', (e) => {
					if (e.lengthComputable) {
						uploadedBytes = e.loaded;
						totalBytes = e.total;
						uploadProgress = Math.round((e.loaded / e.total) * 100);

						// Calculate speed (bytes per second)
						const elapsedMs = Date.now() - uploadStartTime;
						if (elapsedMs > 0) {
							uploadSpeed = (e.loaded / elapsedMs) * 1000;
						}
					}
				});

				xhr.addEventListener('load', () => {
					if (xhr.status >= 200 && xhr.status < 300) {
						try {
							resolve(JSON.parse(xhr.responseText));
						} catch {
							reject(new Error('Invalid response from server'));
						}
					} else {
						reject(new Error(`Upload failed: ${xhr.status} - ${xhr.responseText}`));
					}
				});

				xhr.addEventListener('error', () => {
					reject(new Error('Network error during upload'));
				});

				xhr.addEventListener('abort', () => {
					reject(new Error('Upload cancelled'));
				});

				xhr.open('POST', `${apiBaseUrl}/upload`);
				xhr.setRequestHeader('Authorization', `Bearer ${token}`);
				xhr.send(formData);
			});

			// Verify hash matches
			if (uploadData.hash !== localHash) {
				showToast('File verification failed. Hash mismatch.');
				return;
			}

			// Update draft with upload info
			const updateReq: UpdateDraftRequest = {
				upload_id: uploadData.id,
				file_sha256: uploadData.hash,
				file_sha1: uploadData.sha1,
				file_size: uploadData.size,
				filename: file.name
			};

			const currentDraft = draft;
			if (!currentDraft) {
				throw new Error('Draft missing');
			}
			const updateRes = await fetch(`${apiBaseUrl}/draft/${currentDraft.draft_id}`, {
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
			uploadProgress = 0;
			uploadedBytes = 0;
			totalBytes = 0;
			uploadSpeed = 0;
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
			await goto(resolve('/my-wads'), { invalidateAll: true });
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Publish failed';
			showToast(message);
		} finally {
			publishing = false;
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
	<div class="mb-8 flex items-center justify-between">
		<h1 class="text-3xl font-semibold tracking-tight">
			{isPublished ? 'Edit WAD' : 'Upload WAD'}
		</h1>
		<a
			href={resolve('/my-wads')}
			class="text-sm text-zinc-400 transition-colors hover:text-zinc-200"
		>
			← Back to My WADs
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
			<p class="mb-4 text-zinc-400">Please log in to upload WADs.</p>
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
			<a
				href={resolve('/upload')}
				class="mt-4 inline-block rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700"
			>
				Retry
			</a>
		</div>
	{:else}
		<div class="space-y-6">
			<!-- File Upload Section -->
			<div class="rounded-lg bg-zinc-900/60 p-6 ring-1 ring-zinc-800">
				<h2 class="mb-4 text-lg font-semibold">WAD File</h2>

				{#if uploadedFile}
					<div class="flex items-center justify-between rounded-lg bg-zinc-800/50 p-4">
						<div class="flex items-center gap-3">
							<svg
								class="h-8 w-8 text-green-400"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
								/>
							</svg>
							<div>
								<p class="font-medium text-zinc-100">{uploadedFile.name}</p>
								<p class="text-sm text-zinc-400">{humanBytes(uploadedFile.size)}</p>
							</div>
						</div>
						{#if !isPublished}
							<label
								class="cursor-pointer rounded-lg bg-zinc-700 px-3 py-1.5 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-600"
							>
								Replace
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
				{:else}
					<label
						class="flex w-full cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed border-zinc-700 transition-colors hover:border-zinc-500"
						class:h-40={!uploading}
						class:py-6={uploading}
						class:cursor-default={uploading}
					>
						{#if uploading}
							<div class="w-full space-y-4 px-6">
								<!-- Progress Header -->
								<div class="flex items-center justify-between">
									<span class="text-sm font-medium text-zinc-200">Uploading...</span>
									<span class="text-sm font-semibold text-red-400">{uploadProgress}%</span>
								</div>

								<!-- Progress Bar -->
								<div class="h-2 w-full overflow-hidden rounded-full bg-zinc-800">
									<div
										class="h-full bg-gradient-to-r from-red-600 to-red-500 transition-all duration-150 ease-out"
										style="width: {uploadProgress}%"
									></div>
								</div>

								<!-- Stats Grid -->
								<div class="grid grid-cols-3 gap-4 text-center">
									<div class="rounded-lg bg-zinc-800/50 px-3 py-2">
										<p class="text-xs tracking-wide text-zinc-500 uppercase">Uploaded</p>
										<p class="text-sm font-medium text-zinc-200">{humanBytes(uploadedBytes)}</p>
										<p class="text-xs text-zinc-500">of {humanBytes(totalBytes)}</p>
									</div>
									<div class="rounded-lg bg-zinc-800/50 px-3 py-2">
										<p class="text-xs tracking-wide text-zinc-500 uppercase">Speed</p>
										<p class="text-sm font-medium text-zinc-200">{formatSpeed(uploadSpeed)}</p>
										<p class="text-xs text-zinc-500">average</p>
									</div>
									<div class="rounded-lg bg-zinc-800/50 px-3 py-2">
										<p class="text-xs tracking-wide text-zinc-500 uppercase">Remaining</p>
										<p class="text-sm font-medium text-zinc-200">
											{formatEta(estimatedTimeRemaining())}
										</p>
										<p class="text-xs text-zinc-500">estimated</p>
									</div>
								</div>
							</div>
						{:else}
							<div class="flex flex-col items-center">
								<svg
									class="mb-2 h-10 w-10 text-zinc-500"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="2"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
									/>
								</svg>
								<p class="mb-1 text-sm text-zinc-400">Click to upload or drag and drop</p>
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
						<label for="author" class="mb-1 block text-sm font-medium text-zinc-300"
							>Authors <span class="text-xs font-normal text-zinc-500"
								>(comma-separated list of authors)</span
							></label
						>
						<input
							id="author"
							type="text"
							bind:value={author}
							placeholder="Your name or handle"
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

					<div class="flex items-center gap-3">
						<input
							id="ai-enabled"
							type="checkbox"
							bind:checked={aiEnabled}
							class="h-4 w-4 rounded border-zinc-700 bg-zinc-800 text-red-500 focus:ring-red-500 focus:ring-offset-zinc-900"
						/>
						<label for="ai-enabled" class="text-sm text-zinc-300">
							Enable AI Analysis
							<span class="ml-1 text-zinc-500">(generates descriptions and tags automatically)</span
							>
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
						class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-zinc-200 transition-colors hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
					>
						{#if saving}
							<div
								class="h-4 w-4 animate-spin rounded-full border-2 border-zinc-600 border-t-zinc-200"
							></div>
							Saving...
						{:else}
							Save
						{/if}
					</button>

					<button
						type="button"
						onclick={deleteDraft}
						disabled={deleting}
						class="inline-flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2 text-sm font-semibold text-red-400 transition-colors hover:bg-zinc-700 hover:text-red-300 disabled:cursor-not-allowed disabled:opacity-50"
					>
						{#if deleting}
							<div
								class="h-4 w-4 animate-spin rounded-full border-2 border-zinc-600 border-t-red-400"
							></div>
							Deleting...
						{:else}
							Delete
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
						class="inline-flex items-center gap-2 rounded-lg bg-red-900/70 px-6 py-2 text-sm font-semibold text-white transition-colors hover:bg-red-800/70 disabled:cursor-not-allowed disabled:opacity-50"
					>
						{#if publishing}
							<div
								class="h-4 w-4 animate-spin rounded-full border-2 border-red-300 border-t-white"
							></div>
							Publishing...
						{:else}
							Publish WAD
						{/if}
					</button>
				{:else}
					<span class="rounded bg-green-900/50 px-3 py-1.5 text-sm font-medium text-green-300">
						✓ Published
					</span>
				{/if}
			</div>

			{#if !canPublish && !isPublished}
				<p class="text-center text-sm text-zinc-500">Upload a file to enable publishing.</p>
			{/if}
		</div>
	{/if}
</section>
