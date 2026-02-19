<svelte:head>
	<title>ACCOUNT - ɢɪʙ.ɢɢ</title>
</svelte:head>

<script lang="ts">
	import { onMount } from 'svelte';
	import { showToast } from '$lib/stores/toast';
	import type { UserProfileFull, UserProfileView } from '$lib/types/wadinfo';

	let loading = $state(true);
	let saving = $state(false);
	let notAuthenticated = $state(false);
	let loadError = $state<string | null>(null);

	let profile = $state<UserProfileView | null>(null);
	let username = $state('');
	let avatarUrl = $state('');
	let privacyHideActivity = $state(false);

	function isFullProfile(value: UserProfileView | null): value is UserProfileFull {
		return !!value && 'privacy_hide_activity' in value;
	}

	function formatEpochMs(value: number | null | undefined): string {
		if (!value || !Number.isFinite(value)) return '—';
		const dt = new Date(value);
		if (Number.isNaN(dt.getTime())) return '—';
		return dt.toLocaleString();
	}

	function syncFormFromProfile(next: UserProfileView) {
		username = next.username ?? '';
		avatarUrl = next.avatar_url ?? '';
		privacyHideActivity = isFullProfile(next) ? !!next.privacy_hide_activity : false;
	}

	async function loadProfile() {
		loading = true;
		loadError = null;
		notAuthenticated = false;
		try {
			const res = await fetch('/api/account/profile', {
				method: 'GET',
				headers: { accept: 'application/json' }
			});
			if (res.status === 401) {
				notAuthenticated = true;
				profile = null;
				return;
			}
			if (!res.ok) {
				let msg = 'Failed to load account profile.';
				try {
					const body = (await res.json()) as { error?: string };
					if (typeof body?.error === 'string' && body.error.length > 0) msg = body.error;
				} catch {
					// ignore
				}
				throw new Error(msg);
			}
			const next = (await res.json()) as UserProfileView;
			profile = next;
			syncFormFromProfile(next);
		} catch (e: any) {
			loadError = e?.message ?? 'Failed to load account profile.';
		} finally {
			loading = false;
		}
	}

	const hasChanges = $derived.by(() => {
		if (!profile) return false;
		const nextUsername = username.trim();
		const nextAvatar = avatarUrl.trim();
		const currentAvatar = profile.avatar_url ?? '';
		const sameUsername = nextUsername === profile.username;
		const sameAvatar = nextAvatar === currentAvatar;
		const samePrivacy = !isFullProfile(profile) || privacyHideActivity === !!profile.privacy_hide_activity;
		return !(sameUsername && sameAvatar && samePrivacy);
	});

	const canSave = $derived.by(() => {
		if (!profile || saving) return false;
		if (!username.trim()) return false;
		return hasChanges;
	});

	function onReset() {
		if (!profile) return;
		syncFormFromProfile(profile);
	}

	async function onSave(e: SubmitEvent) {
		e.preventDefault();
		if (!profile || !canSave) return;
		saving = true;
		try {
			const res = await fetch('/api/account/profile', {
				method: 'PUT',
				headers: {
					'content-type': 'application/json',
					accept: 'application/json'
				},
				body: JSON.stringify({
					username: username.trim(),
					avatar_url: avatarUrl.trim() || null,
					privacy_hide_activity: privacyHideActivity
				})
			});

			if (res.status === 401) {
				notAuthenticated = true;
				showToast('Please sign in to update your account.');
				return;
			}
			if (!res.ok) {
				let msg = 'Failed to save account settings.';
				try {
					const body = (await res.json()) as { error?: string };
					if (typeof body?.error === 'string' && body.error.length > 0) msg = body.error;
				} catch {
					// ignore
				}
				throw new Error(msg);
			}

			const updated = (await res.json()) as UserProfileFull;
			profile = updated;
			syncFormFromProfile(updated);
			showToast('Account updated.');
		} catch (err: any) {
			showToast(err?.message ?? 'Failed to save account settings.');
		} finally {
			saving = false;
		}
	}

	onMount(loadProfile);
</script>

<section class="mx-auto w-full max-w-5xl px-4 py-8">
	<div class="mb-6">
		<h1 class="text-3xl font-semibold tracking-tight text-zinc-100">Account Management</h1>
		<p class="mt-2 text-sm text-zinc-400">
			Manage your public profile identity and privacy preferences.
		</p>
	</div>

	{#if loading}
		<div class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6">
			<p class="text-sm text-zinc-300">Loading profile…</p>
		</div>
	{:else if notAuthenticated}
		<div class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6">
			<h2 class="text-lg font-semibold text-zinc-100">Sign in required</h2>
			<p class="mt-2 text-sm text-zinc-400">
				You need to be signed in to view and edit account settings.
			</p>
			<a
				href="#login"
				class="mt-4 inline-flex rounded-lg bg-red-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-red-700"
			>
				Open login
			</a>
		</div>
	{:else if loadError || !profile}
		<div class="rounded-xl border border-red-900/60 bg-zinc-950/80 p-6">
			<h2 class="text-lg font-semibold text-zinc-100">Unable to load profile</h2>
			<p class="mt-2 text-sm text-zinc-400">{loadError ?? 'Please try again in a moment.'}</p>
			<button
				type="button"
				onclick={loadProfile}
				class="mt-4 cursor-pointer rounded-lg border border-zinc-700 px-4 py-2 text-sm font-semibold text-zinc-200 transition hover:bg-zinc-900"
			>
				Retry
			</button>
		</div>
	{:else}
		<div class="grid gap-6 lg:grid-cols-[1fr_2fr]">
			<aside class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6">
				<div class="mx-auto flex h-24 w-24 items-center justify-center overflow-hidden rounded-full bg-zinc-900 ring-1 ring-zinc-700">
					{#if avatarUrl.trim()}
						<img
							src={avatarUrl.trim()}
							alt="Profile avatar"
							class="h-full w-full object-cover"
							onerror={(e) => {
								(e.currentTarget as HTMLImageElement).style.display = 'none';
							}}
						/>
					{:else}
						<span class="text-3xl font-semibold text-zinc-300">{(username.trim()[0] ?? 'U').toUpperCase()}</span>
					{/if}
				</div>
				<div class="mt-4 text-center">
					<p class="text-base font-semibold text-zinc-100">{profile.username}</p>
					<p class="mt-1 text-xs text-zinc-500">User ID</p>
					<p class="mt-1 break-all text-xs text-zinc-400">{profile.id}</p>
				</div>

				<div class="mt-6 space-y-4 border-t border-zinc-800 pt-5 text-sm">
					<div>
						<p class="text-xs font-semibold tracking-wide text-zinc-500">REGISTERED</p>
						<p class="mt-1 text-zinc-300">{formatEpochMs(profile.registered_at)}</p>
					</div>
					<div>
						<p class="text-xs font-semibold tracking-wide text-zinc-500">LAST ACTIVE</p>
						<p class="mt-1 text-zinc-300">
							{formatEpochMs(profile.last_active_at ?? null)}
						</p>
					</div>
				</div>
			</aside>

			<form class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6" onsubmit={onSave}>
				<div class="mb-5">
					<h2 class="text-xl font-semibold text-zinc-100">Profile Settings</h2>
					<p class="mt-1 text-sm text-zinc-400">Choose what others can see and update your identity details.</p>
				</div>

				<div class="space-y-5">
					<label class="block">
						<span class="text-xs font-semibold tracking-wide text-zinc-300">USERNAME</span>
						<input
							type="text"
							bind:value={username}
							autocomplete="username"
							required
							maxlength="30"
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
						/>
						<p class="mt-1 text-xs text-zinc-500">3–30 characters. Letters, numbers, dots, dashes, and underscores are recommended.</p>
					</label>

					<label class="block">
						<span class="text-xs font-semibold tracking-wide text-zinc-300">AVATAR URL</span>
						<input
							type="url"
							bind:value={avatarUrl}
							placeholder="https://example.com/avatar.png"
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
						/>
						<p class="mt-1 text-xs text-zinc-500">Leave blank to remove your avatar.</p>
					</label>

					{#if isFullProfile(profile)}
						<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
							<label class="flex cursor-pointer items-start gap-3">
								<input
									type="checkbox"
									bind:checked={privacyHideActivity}
									class="mt-0.5 h-4 w-4 rounded border-zinc-700 bg-zinc-950 text-red-600 focus:ring-red-700"
								/>
								<span>
									<span class="text-sm font-semibold text-zinc-200">Hide my activity from other users</span>
									<span class="mt-1 block text-xs text-zinc-500">
										When enabled, others won’t see your last active time in your public profile.
									</span>
								</span>
							</label>
						</div>
					{/if}
				</div>

				<div class="mt-6 flex flex-wrap items-center gap-3 border-t border-zinc-800 pt-5">
					<button
						type="submit"
						disabled={!canSave}
						class="cursor-pointer rounded-lg bg-red-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-60"
					>
						{saving ? 'Saving…' : 'Save changes'}
					</button>
					<button
						type="button"
						onclick={onReset}
						disabled={!hasChanges || saving}
						class="cursor-pointer rounded-lg border border-zinc-700 px-4 py-2 text-sm font-semibold text-zinc-200 transition hover:bg-zinc-900 disabled:cursor-not-allowed disabled:opacity-60"
					>
						Discard
					</button>
					{#if hasChanges && !saving}
						<p class="text-xs text-zinc-500">You have unsaved changes.</p>
					{/if}
				</div>
			</form>
		</div>
	{/if}
</section>
