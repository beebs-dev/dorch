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

	// Track initial state for change detection
	let initialTitle = data.wad?.meta?.title ?? '';
	let hasUnsavedChanges = $derived(title !== initialTitle);

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
		const token = getAccessToken();
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
				body: JSON.stringify({ title: title.trim() || null })
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
			initialTitle = title; // Reset change tracking
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

<main class="edit-wad-page">
	{#if authChecking}
		<div class="loading">Checking authentication...</div>
	{:else if notAuthenticated}
		<div class="auth-required">
			<h1>Authentication Required</h1>
			<p>You must be logged in to edit WADs.</p>
			<a href={resolve('/account')} class="login-link">Log in</a>
		</div>
	{:else if !data.wad}
		<div class="error">WAD not found.</div>
	{:else if !isOwner()}
		<div class="error">
			<h1>Not Authorized</h1>
			<p>You can only edit WADs that you uploaded.</p>
			<a href={resolve(`/wad/${data.wadId}`)} class="back-link">Back to WAD</a>
		</div>
	{:else}
		<div class="edit-form-container">
			<h1>Edit WAD</h1>
			<p class="wad-info">
				Editing: <strong>{wadLabel(data.wad.meta)}</strong>
			</p>

			<form onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
				<div class="form-group">
					<label for="title">Title</label>
					<input
						type="text"
						id="title"
						bind:value={title}
						placeholder="Enter WAD title"
						maxlength="255"
					/>
					<p class="help-text">The display title for this WAD. Leave empty to use the filename.</p>
				</div>

				<div class="form-actions">
					<button type="button" class="cancel-btn" onclick={handleCancel} disabled={saving}>
						Cancel
					</button>
					<button type="submit" class="save-btn" disabled={saving || !hasUnsavedChanges}>
						{#if saving}
							Saving...
						{:else}
							Save Changes
						{/if}
					</button>
				</div>
			</form>
		</div>
	{/if}
</main>

<style>
	.edit-wad-page {
		max-width: 800px;
		margin: 0 auto;
		padding: 2rem;
	}

	.loading,
	.auth-required,
	.error {
		text-align: center;
		padding: 3rem;
	}

	.auth-required h1,
	.error h1 {
		margin-bottom: 1rem;
	}

	.login-link,
	.back-link {
		display: inline-block;
		margin-top: 1rem;
		padding: 0.75rem 1.5rem;
		background: var(--color-accent, #0066cc);
		color: white;
		border-radius: 6px;
		text-decoration: none;
	}

	.login-link:hover,
	.back-link:hover {
		background: var(--color-accent-hover, #0052a3);
	}

	.edit-form-container h1 {
		margin-bottom: 0.5rem;
	}

	.wad-info {
		color: var(--color-text-secondary, #666);
		margin-bottom: 2rem;
	}

	.form-group {
		margin-bottom: 1.5rem;
	}

	.form-group label {
		display: block;
		margin-bottom: 0.5rem;
		font-weight: 500;
	}

	.form-group input {
		width: 100%;
		padding: 0.75rem;
		font-size: 1rem;
		border: 1px solid var(--color-border, #ccc);
		border-radius: 6px;
		background: var(--color-input-bg, #fff);
		color: var(--color-text, #333);
	}

	.form-group input:focus {
		outline: none;
		border-color: var(--color-accent, #0066cc);
		box-shadow: 0 0 0 2px rgba(0, 102, 204, 0.2);
	}

	.help-text {
		margin-top: 0.5rem;
		font-size: 0.875rem;
		color: var(--color-text-secondary, #666);
	}

	.form-actions {
		display: flex;
		gap: 1rem;
		justify-content: flex-end;
		margin-top: 2rem;
		padding-top: 1.5rem;
		border-top: 1px solid var(--color-border, #eee);
	}

	.cancel-btn,
	.save-btn {
		padding: 0.75rem 1.5rem;
		font-size: 1rem;
		border-radius: 6px;
		cursor: pointer;
		transition: background-color 0.2s;
	}

	.cancel-btn {
		background: var(--color-bg-secondary, #f0f0f0);
		color: var(--color-text, #333);
		border: 1px solid var(--color-border, #ccc);
	}

	.cancel-btn:hover:not(:disabled) {
		background: var(--color-bg-tertiary, #e0e0e0);
	}

	.save-btn {
		background: var(--color-accent, #0066cc);
		color: white;
		border: none;
	}

	.save-btn:hover:not(:disabled) {
		background: var(--color-accent-hover, #0052a3);
	}

	.save-btn:disabled,
	.cancel-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}
</style>
