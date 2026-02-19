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
	let privacyHideActivity = $state(false);
	let avatarFile = $state<File | null>(null);
	let uploadingAvatar = $state(false);

	const maxAvatarUploadBytes = 5 * 1024 * 1024;
	const accessTokenCookieName = 'dorch_access_token';
	const accessTokenExpCookieName = 'dorch_access_token_expires_at';
	const apiBaseUrl = 'https://api.gib.gg';
	const accessTokenComfortWindowMs = 60_000;

	function readCookie(name: string): string | null {
		if (typeof document === 'undefined') return null;
		const encodedPrefix = `${encodeURIComponent(name)}=`;
		const parts = document.cookie.split(';');
		for (const rawPart of parts) {
			const part = rawPart.trim();
			if (!part.startsWith(encodedPrefix)) continue;
			const rawValue = part.slice(encodedPrefix.length);
			try {
				return decodeURIComponent(rawValue);
			} catch {
				return rawValue;
			}
		}
		return null;
	}

	function getAccessToken(): string | null {
		const token = readCookie(accessTokenCookieName);
		if (!token) return null;
		const trimmed = token.trim();
		return trimmed.length > 0 ? trimmed : null;
	}

	function decodeJwtPayload(token: string): Record<string, unknown> | null {
		const parts = token.split('.');
		if (parts.length < 2) return null;
		try {
			const payload = parts[1].replace(/-/g, '+').replace(/_/g, '/');
			const padLength = payload.length % 4;
			const paddedPayload = payload + (padLength ? '='.repeat(4 - padLength) : '');
			const jsonPayload = atob(paddedPayload);
			const parsed = JSON.parse(jsonPayload);
			return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : null;
		} catch {
			return null;
		}
	}

	function parseIsoDateMs(value: string | null): number | null {
		if (!value) return null;
		const parsed = Date.parse(value);
		return Number.isFinite(parsed) ? parsed : null;
	}

	function getTokenExpiryMs(token: string): number | null {
		const payload = decodeJwtPayload(token);
		const exp = payload?.exp;
		const jwtExpiryMs = typeof exp === 'number' && Number.isFinite(exp) ? exp * 1000 : null;
		const cookieExpiryMs = parseIsoDateMs(readCookie(accessTokenExpCookieName));

		if (jwtExpiryMs !== null && cookieExpiryMs !== null) {
			return Math.min(jwtExpiryMs, cookieExpiryMs);
		}
		return jwtExpiryMs ?? cookieExpiryMs;
	}

	function isTokenComfortablyValid(token: string): boolean {
		const expiryMs = getTokenExpiryMs(token);
		if (expiryMs === null) return false;
		return Date.now() + accessTokenComfortWindowMs < expiryMs;
	}

	async function refreshAccessTokenIfPossible(): Promise<string | null> {
		const res = await fetch('/api/session', {
			method: 'GET',
			headers: { accept: 'application/json' }
		});
		if (res.status === 401) {
			notAuthenticated = true;
			return null;
		}
		if (!res.ok) return null;
		return getAccessToken();
	}

	async function ensureComfortableAccessToken(): Promise<string | null> {
		const token = getAccessToken();
		if (token && isTokenComfortablyValid(token)) return token;

		const refreshedToken = await refreshAccessTokenIfPossible();
		if (!refreshedToken) return null;
		return isTokenComfortablyValid(refreshedToken) ? refreshedToken : null;
	}

	async function directApiWrite(path: string, init: RequestInit): Promise<Response> {
		const buildAuthHeaders = (token: string) => {
			const headers = new Headers(init.headers);
			headers.set('authorization', `Bearer ${token}`);
			if (!headers.has('accept')) headers.set('accept', 'application/json');
			return headers;
		};

		let token = await ensureComfortableAccessToken();
		if (!token) {
			return new Response(JSON.stringify({ error: 'Not authenticated' }), {
				status: 401,
				headers: { 'content-type': 'application/json' }
			});
		}

		const url = `${apiBaseUrl}${path}`;
		let res = await fetch(url, {
			...init,
			headers: buildAuthHeaders(token)
		});

		if (res.status !== 401) return res;

		token = await refreshAccessTokenIfPossible();
		if (!token) return res;

		res = await fetch(url, {
			...init,
			headers: buildAuthHeaders(token)
		});
		return res;
	}

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
		privacyHideActivity = isFullProfile(next) ? !!next.privacy_hide_activity : false;
		avatarFile = null;
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
		const sameUsername = nextUsername === profile.username;
		const samePrivacy = !isFullProfile(profile) || privacyHideActivity === !!profile.privacy_hide_activity;
		return !(sameUsername && samePrivacy);
	});

	const canSave = $derived.by(() => {
		if (!profile || saving || uploadingAvatar) return false;
		if (!username.trim()) return false;
		return hasChanges;
	});

	const canUploadAvatar = $derived.by(() => {
		return !!profile && !!avatarFile && !saving && !uploadingAvatar;
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
			const res = await directApiWrite(`/user/profile/${encodeURIComponent(profile.id)}`, {
				method: 'PUT',
				headers: {
					'content-type': 'application/json',
					accept: 'application/json'
				},
				body: JSON.stringify({
					username: username.trim(),
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

	function onAvatarSelected(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const nextFile = input.files?.[0] ?? null;
		if (!nextFile) {
			avatarFile = null;
			return;
		}
		if (!nextFile.type.toLowerCase().startsWith('image/')) {
			showToast('Avatar must be an image file.');
			avatarFile = null;
			input.value = '';
			return;
		}
		if (nextFile.size > maxAvatarUploadBytes) {
			showToast(`Avatar exceeds max size of ${maxAvatarUploadBytes} bytes.`);
			avatarFile = null;
			input.value = '';
			return;
		}
		avatarFile = nextFile;
	}

	async function onAvatarUpload() {
		if (!profile || !avatarFile || !canUploadAvatar) return;
		uploadingAvatar = true;
		try {
			const res = await directApiWrite(`/user/profile/${encodeURIComponent(profile.id)}/avatar`, {
				method: 'PUT',
				headers: {
					'content-type': avatarFile.type || 'application/octet-stream',
					accept: 'application/json'
				},
				body: avatarFile
			});
			if (res.status === 401) {
				notAuthenticated = true;
				showToast('Please sign in to upload an avatar.');
				return;
			}
			if (!res.ok) {
				let msg = 'Failed to upload avatar.';
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
			showToast('Avatar updated.');
		} catch (err: any) {
			showToast(err?.message ?? 'Failed to upload avatar.');
		} finally {
			uploadingAvatar = false;
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
					{#if profile.avatar_url}
						<img
							src={profile.avatar_url}
							alt="Profile avatar"
							class="h-full w-full object-cover"
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
					<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
						<p class="text-xs font-semibold tracking-wide text-zinc-300">AVATAR IMAGE</p>
						<p class="mt-1 text-xs text-zinc-500">
							Upload any image up to {maxAvatarUploadBytes} bytes. It will be resized and stored as webp.
						</p>
						<div class="mt-3 flex flex-wrap items-center gap-3">
							<input
								type="file"
								accept="image/*"
								onchange={onAvatarSelected}
								class="block w-full max-w-sm cursor-pointer rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-xs text-zinc-300 file:mr-3 file:cursor-pointer file:rounded-md file:border-0 file:bg-zinc-800 file:px-3 file:py-1.5 file:text-xs file:font-semibold file:text-zinc-100 hover:file:bg-zinc-700"
							/>
							<button
								type="button"
								onclick={onAvatarUpload}
								disabled={!canUploadAvatar}
								class="cursor-pointer rounded-lg border border-zinc-700 px-4 py-2 text-sm font-semibold text-zinc-200 transition hover:bg-zinc-900 disabled:cursor-not-allowed disabled:opacity-60"
							>
								{uploadingAvatar ? 'Uploading…' : 'Upload avatar'}
							</button>
						</div>
						{#if avatarFile}
							<p class="mt-2 text-xs text-zinc-500">Selected: {avatarFile.name}</p>
						{/if}
					</div>

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
