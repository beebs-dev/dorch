<script lang="ts">
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	function formatEpochMs(value: number | null | undefined): string {
		if (!value || !Number.isFinite(value)) return '—';
		const dt = new Date(value);
		if (Number.isNaN(dt.getTime())) return '—';
		return dt.toLocaleString();
	}

	const profile = $derived(() => data.profile);
	const profileInitial = $derived(() => {
		const display = profile().display_name?.trim();
		const username = profile().username?.trim();
		return (display?.[0] ?? username?.[0] ?? 'U').toUpperCase();
	});
</script>

<svelte:head>
	<title>{data.profile.display_name || data.profile.username} - ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-5xl px-4 py-8">
	<div class="mb-6">
		<h1 class="text-3xl font-semibold tracking-tight text-zinc-100">Public Profile</h1>
		<p class="mt-2 text-sm text-zinc-400">Read-only account details shared with the community.</p>
	</div>

	<div class="grid gap-6 lg:grid-cols-[1fr_2fr]">
		<aside class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6">
			<div
				class="mx-auto flex h-28 w-28 items-center justify-center overflow-hidden rounded-full bg-zinc-900 ring-1 ring-zinc-700"
			>
				{#if profile().avatar_url}
					<img
						src={profile().avatar_url}
						alt={`${profile().username} avatar`}
						class="h-full w-full object-cover"
					/>
				{:else}
					<span class="text-3xl font-semibold text-zinc-300">{profileInitial()}</span>
				{/if}
			</div>

			<div class="mt-5 text-center">
				<p class="text-xl font-semibold text-zinc-100">{profile().display_name}</p>
				<p class="mt-1 text-sm text-zinc-400">@{profile().username}</p>
			</div>
		</aside>

		<div class="rounded-xl border border-zinc-800 bg-zinc-950/80 p-6">
			<h2 class="text-xl font-semibold text-zinc-100">Profile Details</h2>
			<p class="mt-1 text-sm text-zinc-400">These details are visible to other users.</p>

			<div class="mt-5 grid gap-4 sm:grid-cols-2">
				<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
					<p class="text-xs font-semibold tracking-wide text-zinc-500">USERNAME</p>
					<p class="mt-2 font-[var(--dorch-mono)] break-all text-zinc-200">@{profile().username}</p>
				</div>

				<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
					<p class="text-xs font-semibold tracking-wide text-zinc-500">DISPLAY NAME</p>
					<p class="mt-2 break-words text-zinc-200">{profile().display_name}</p>
				</div>

				<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4 sm:col-span-2">
					<p class="text-xs font-semibold tracking-wide text-zinc-500">USER ID</p>
					<p class="mt-2 font-[var(--dorch-mono)] break-all text-zinc-300">{profile().id}</p>
				</div>

				<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
					<p class="text-xs font-semibold tracking-wide text-zinc-500">REGISTERED</p>
					<p class="mt-2 text-zinc-200">{formatEpochMs(profile().registered_at)}</p>
				</div>

				<div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-4">
					<p class="text-xs font-semibold tracking-wide text-zinc-500">LAST ACTIVE</p>
					<p class="mt-2 text-zinc-200">{formatEpochMs(profile().last_active_at ?? null)}</p>
				</div>
			</div>
		</div>
	</div>
</section>
