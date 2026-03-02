<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { showToast } from '$lib/stores/toast';
	import { login, subscribe as authSubscribe, type AuthState } from '$lib/stores/auth';

	const apiBaseUrl = 'https://api.gib.gg';

	let username = $state('');
	let email = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let showPasswords = $state(false);
	let submitting = $state(false);
	let authState = $state<AuthState>({
		isAuthenticated: false,
		userId: null,
		username: null,
		accessToken: null
	});

	onMount(() =>
		authSubscribe((next) => {
			authState = next;
		})
	);

	const canSubmit = $derived.by(() => {
		if (submitting) return false;
		if (!username.trim() || !email.trim() || !password || !confirmPassword) return false;
		return true;
	});

	function readErrorMessage(body: unknown): string | null {
		if (typeof body !== 'object' || body === null) return null;
		const candidate = body as { error?: unknown; message?: unknown };
		if (typeof candidate.error === 'string' && candidate.error.length > 0) return candidate.error;
		if (typeof candidate.message === 'string' && candidate.message.length > 0)
			return candidate.message;
		return null;
	}

	async function onSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!canSubmit) return;

		if (password !== confirmPassword) {
			showToast('Passwords do not match.');
			return;
		}

		submitting = true;
		try {
			const payload = {
				username: username.trim(),
				email: email.trim(),
				password
			};

			const registerRes = await fetch(`${apiBaseUrl}/user/register`, {
				method: 'POST',
				headers: {
					'content-type': 'application/json',
					accept: 'application/json'
				},
				body: JSON.stringify(payload)
			});

			if (!registerRes.ok) {
				let message = 'Unable to create account.';
				try {
					message = readErrorMessage(await registerRes.json()) ?? message;
				} catch {
					// ignore
				}
				throw new Error(message);
			}

			await login(payload.username, password, true);
			showToast('Account created. Welcome to ɢɪʙ.ɢɢ.');
			await goto(resolve('/account'));
		} catch (err: any) {
			showToast(err?.message ?? 'Unable to create account.');
		} finally {
			submitting = false;
		}
	}
</script>

<svelte:head>
	<title>REGISTER - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-3xl px-4 py-8">
	<div class="mb-6">
		<h1 class="text-3xl font-semibold tracking-tight text-zinc-100">Create your account</h1>
		<p class="mt-2 text-sm text-zinc-400">
			Set up your ɢɪʙ.ɢɢ account to create and manage custom servers.
		</p>
	</div>

	{#if authState.isAuthenticated}
		<div class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6">
			<h2 class="text-lg font-semibold text-zinc-100">You’re already signed in</h2>
			<p class="mt-2 text-sm text-zinc-400">
				Continue to your account settings to manage your profile.
			</p>
			<a
				href={resolve('/account')}
				class="mt-4 inline-flex rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition hover:bg-red-800/70"
			>
				Go to account
			</a>
		</div>
	{:else}
		<form class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6" onsubmit={onSubmit}>
			<div class="grid gap-4 sm:grid-cols-2">
				<label class="block sm:col-span-2">
					<span class="text-xs font-semibold tracking-wide text-zinc-300">USERNAME</span>
					<input
						type="text"
						bind:value={username}
						autocomplete="username"
						required
						maxlength="50"
						class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
						placeholder="doomguy"
					/>
				</label>

				<label class="block sm:col-span-2">
					<span class="text-xs font-semibold tracking-wide text-zinc-300">EMAIL</span>
					<input
						type="email"
						bind:value={email}
						autocomplete="email"
						required
						maxlength="254"
						class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
						placeholder="you@example.com"
					/>
				</label>

				<label class="block sm:col-span-2">
					<span class="text-xs font-semibold tracking-wide text-zinc-300">PASSWORD</span>
					<div class="relative mt-2">
						<input
							type={showPasswords ? 'text' : 'password'}
							bind:value={password}
							autocomplete="new-password"
							required
							minlength="8"
							class="w-full rounded-lg bg-zinc-950 py-2 pr-14 pl-3 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="••••••••"
						/>
						<button
							type="button"
							onclick={() => (showPasswords = !showPasswords)}
							class="absolute inset-y-0 right-0 my-1 mr-1 cursor-pointer rounded-md px-3 text-xs font-semibold text-zinc-300 transition hover:bg-zinc-900 hover:text-zinc-100 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
							aria-label={showPasswords ? 'Hide passwords' : 'Show passwords'}
						>
							{showPasswords ? 'HIDE' : 'SHOW'}
						</button>
					</div>
				</label>

				<label class="block sm:col-span-2">
					<span class="text-xs font-semibold tracking-wide text-zinc-300">CONFIRM PASSWORD</span>
					<div class="relative mt-2">
						<input
							type={showPasswords ? 'text' : 'password'}
							bind:value={confirmPassword}
							autocomplete="new-password"
							required
							minlength="8"
							class="w-full rounded-lg bg-zinc-950 py-2 pr-14 pl-3 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="••••••••"
						/>
						<button
							type="button"
							onclick={() => (showPasswords = !showPasswords)}
							class="absolute inset-y-0 right-0 my-1 mr-1 cursor-pointer rounded-md px-3 text-xs font-semibold text-zinc-300 transition hover:bg-zinc-900 hover:text-zinc-100 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
							aria-label={showPasswords ? 'Hide passwords' : 'Show passwords'}
						>
							{showPasswords ? 'HIDE' : 'SHOW'}
						</button>
					</div>
				</label>
			</div>

			<div class="mt-6 flex flex-wrap items-center gap-3 border-t border-zinc-800 pt-5">
				<button
					type="submit"
					disabled={!canSubmit}
					aria-busy={submitting}
					class="cursor-pointer rounded-lg bg-red-900/70 px-4 py-2 text-sm font-semibold text-white transition hover:bg-red-800/70 disabled:cursor-not-allowed disabled:opacity-60"
				>
					{submitting ? 'Creating account…' : 'Create account'}
				</button>

				<a
					href="#login"
					class="rounded-lg border border-zinc-700 px-4 py-2 text-sm font-semibold text-zinc-200 transition hover:bg-zinc-900"
				>
					Already have an account?
				</a>
			</div>
		</form>
	{/if}
</section>
