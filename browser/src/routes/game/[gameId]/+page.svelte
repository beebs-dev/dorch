<script lang="ts">
	import type { PageData } from './$types';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { wadLabel } from '$lib/utils/format';

	let { data }: { data: PageData } = $props();

	const game = $derived(() => data.game);
	const info = $derived(() => data.game.info);
	const currentMap = $derived(() => data.currentMap);
	const currentMapWadId = $derived(() => data.currentMapWadId);

	const pageTitle = $derived(() => `${info()?.name ?? data.gameId} - DORCH`);
	const videoSrc = $derived(
		() => `https://gibstrim.nyc3.digitaloceanspaces.com/${encodeURIComponent(data.gameId)}/index.m3u8`
	);

	let identity = $state(randomIdent());

	function randomIdent(): string {
		const adjectives = [
			'quick',
			'bright',
			'silent',
			'fierce',
			'brave',
			'clever',
			'lucky',
			'wild',
			'calm',
			'proud'
		];
		const nouns = [
			'tiger',
			'eagle',
			'lion',
			'wolf',
			'panther',
			'hawk',
			'fox',
			'bear',
			'dragon',
			'falcon'
		];

		const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
		const noun = nouns[Math.floor(Math.random() * nouns.length)];
		const number = Math.floor(Math.random() * 1000);
		return `${adj}-${noun}-${number}`;
	}

	function joinUrl(): string {
		return `https://dorch.beebs.dev/play/?g=${encodeURIComponent(data.gameId)}&identity=${encodeURIComponent(identity)}`;
	}

	function difficultyLabel(skill: number | undefined): string {
		switch (skill) {
			case 1:
				return `I'm Too Young to Die`;
			case 2:
				return `Hurt Me Plenty`;
			case 3:
				return `Ultra-Violence`;
			case 4:
				return `Nightmare!`;
			default:
				return 'Unknown';
		}
	}

	function difficultyColor(skill: number | undefined): string {
		switch (skill) {
			case 1:
				return 'text-green-400';
			case 2:
				return 'text-yellow-400';
			case 3:
				return 'text-orange-400';
			case 4:
				return 'text-red-400';
			default:
				return 'text-zinc-400';
		}
	}

	let videoEl = $state<HTMLVideoElement | null>(null);
	let hlsInstance: { destroy(): void } | null = null;
	let hlsError = $state<string | null>(null);

	onMount(() => {
		let destroyed = false;

		(async () => {
			if (!videoEl) return;

			const src = videoSrc();

			// Native HLS support (Safari, iOS).
			if (videoEl.canPlayType('application/vnd.apple.mpegurl')) {
				videoEl.src = src;
				return;
			}

			const mod = await import('hls.js');
			const Hls = mod.default;
			if (destroyed) return;

			if (!Hls.isSupported()) {
				// Some browsers can still play HLS without canPlayType matching.
				videoEl.src = src;
				return;
			}

			const hls = new Hls({ liveSyncDuration: 12, liveMaxLatencyDuration: 30 });
			hlsInstance = hls;
			hls.on(Hls.Events.ERROR, (_evt: unknown, data: any) => {
				// Keep this very lightweight; the video can keep trying.
				if (data?.fatal) {
					hlsError = data?.details ? String(data.details) : 'HLS error';
				}
			});
			hls.loadSource(src);
			hls.attachMedia(videoEl);
		})();

		return () => {
			destroyed = true;
			try {
				hlsInstance?.destroy();
			} finally {
				hlsInstance = null;
			}
		};
	});

	function wadName(wad: PageData['wads'][number]): string {
		return wad.meta ? wadLabel(wad.meta) : wad.id;
	}

	function isCurrentMap(mapName: string): boolean {
		return Boolean(currentMap() && mapName === currentMap());
	}

	function mapHref(wadId: string, mapName: string): string {
		return resolve(`/wad/${encodeURIComponent(wadId)}/maps/${encodeURIComponent(mapName)}`);
	}
</script>

<svelte:head>
	<title>{pageTitle()}</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<header class="flex flex-wrap items-end justify-between gap-4">
		<div class="min-w-0">
			<h1 class="truncate text-2xl font-semibold tracking-tight">
				{info()?.name ?? 'Game'}
			</h1>
			<div class="mt-1 text-sm text-zinc-400 font-[var(--dorch-mono)] tracking-wide">
				game_id: {data.gameId}
			</div>
		</div>
		<div class="flex items-center gap-2">
			<a
				href={resolve('/servers')}
				class="rounded-md bg-zinc-900 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
			>
				Back to servers
			</a>
		</div>
	</header>

	<div class="mt-5 grid grid-cols-1 gap-6 lg:grid-cols-2">
		<!-- Left: basic server info + PLAY -->
		<div class="space-y-4">
			<div class="rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset p-4">
				<div class="flex items-start justify-between gap-4">
					<div class="min-w-0">
						<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">SERVER</div>
						<div class="mt-1 truncate text-lg font-semibold text-zinc-100">
							{info()?.name ?? '(unknown)'}
						</div>
						<div class="mt-1 text-sm {difficultyColor(info()?.skill)}">
							{difficultyLabel(info()?.skill)}
						</div>
					</div>
					<div class="shrink-0">
						<a
							href={joinUrl()}
							class="dorch-play-button inline-flex items-center justify-center rounded-xl bg-red-950/30 px-8 py-4 text-xl font-semibold text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-red-950/45 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
							aria-label="Play"
						>
							PLAY
						</a>
						<div class="mt-2 text-center text-xs text-zinc-500 font-[var(--dorch-mono)] tracking-wide">
							identity: {identity}
						</div>
					</div>
				</div>

				<div class="mt-4 grid grid-cols-2 gap-3">
					<div class="rounded-lg bg-zinc-900/40 p-3 ring-1 ring-red-950/40 ring-inset">
						<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">PLAYERS</div>
						<div class="mt-1 text-lg font-[var(--dorch-mono)] tracking-wide text-zinc-100">
							{info()?.player_count ?? 0}/{info()?.max_players ?? 0}
						</div>
					</div>
					<div class="rounded-lg bg-zinc-900/40 p-3 ring-1 ring-red-950/40 ring-inset">
						<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">KILLS</div>
						<div class="mt-1 text-lg font-[var(--dorch-mono)] tracking-wide text-zinc-100">
							{#if info()}
								{info()!.monster_kill_count}/{info()!.monster_count}
							{:else}
								—
							{/if}
						</div>
					</div>
				</div>

				<div class="mt-4 rounded-lg bg-zinc-900/40 p-3 ring-1 ring-red-950/40 ring-inset">
					<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">CURRENT MAP</div>
					<div class="mt-1 flex flex-wrap items-center gap-2">
						{#if currentMap()}
							{#if currentMapWadId()}
								<a
									href={mapHref(currentMapWadId()!, currentMap()!)}
									class="rounded-md bg-zinc-950 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800/60"
								>
									{currentMap()}
								</a>
								<a
									href={resolve(`/wad/${encodeURIComponent(currentMapWadId()!)}`)}
									class="text-sm text-zinc-300 hover:text-zinc-100"
								>
									(view WAD)
								</a>
							{:else}
								<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
									{currentMap()}
								</div>
							{/if}
						{:else}
							<div class="text-sm text-zinc-400">Unknown</div>
						{/if}
					</div>
				</div>
			</div>

			<div class="rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset p-4">
				<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">WADS</div>
				<div class="mt-3 space-y-3">
					{#each data.wads as wad (wad.id)}
						<div class="rounded-lg bg-zinc-900/35 p-3 ring-1 ring-red-950/40 ring-inset">
							<div class="flex items-center justify-between gap-3">
								<div class="min-w-0">
									<div class="truncate text-sm font-semibold text-zinc-100">
										{wadName(wad)}
									</div>
									<div class="mt-0.5 text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">
										{wad.id}
									</div>
								</div>
								<a
									href={resolve(`/wad/${encodeURIComponent(wad.id)}`)}
									class="shrink-0 rounded-md bg-zinc-950 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800/60"
								>
									Open
								</a>
							</div>
							{#if wad.mapNames.length}
								<details class="mt-2">
									<summary class="cursor-pointer select-none text-xs text-zinc-400 hover:text-zinc-200">
										Maps ({wad.mapNames.length})
									</summary>
									<div class="mt-2 flex flex-wrap gap-2">
										{#each wad.mapNames as mapName (mapName)}
											<a
												href={mapHref(wad.id, mapName)}
												class={`rounded-md px-2 py-1 text-xs font-[var(--dorch-mono)] tracking-wide ring-1 ring-inset hover:bg-zinc-800/50 ${
													isCurrentMap(mapName)
														? 'bg-red-950/30 text-zinc-100 ring-red-900/60'
														: 'bg-zinc-950 text-zinc-200 ring-red-950/40'
												}`}
											>
												{mapName}
											</a>
										{/each}
									</div>
								</details>
							{:else}
								<div class="mt-2 text-xs text-zinc-500">No maps available (or wadinfo unavailable).</div>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		</div>

		<!-- Right: hero video -->
		<div class="space-y-4">
			<div class="rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset overflow-hidden">
				<div class="flex items-center justify-between px-4 py-3 bg-red-950/25">
					<div class="flex items-center gap-2 text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-200">
						<span
							class="inline-block h-2.5 w-2.5 rounded-full bg-red-500 ring-2 ring-red-200/20"
							aria-label="Recording"
							title="Recording"
						></span>
						<span>LIVE VIEW</span>
					</div>
					<div class="text-xs text-zinc-400 font-[var(--dorch-mono)] tracking-wide">
						{#if data.fetchedAt}
							updated: {new Date(data.fetchedAt).toLocaleTimeString()}
						{/if}
					</div>
				</div>
				<div class="relative">
					<video
						bind:this={videoEl}
						class="aspect-video w-full bg-black"
						autoplay
						muted
						playsinline
						controls
					></video>
					{#if hlsError}
						<div class="absolute left-3 bottom-3 rounded-md bg-zinc-950/80 px-3 py-2 text-xs text-zinc-200 ring-1 ring-red-950/60 ring-inset">
							HLS error: {hlsError}
						</div>
					{/if}
				</div>
			</div>

			<div class="rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset p-4">
				<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">SERVER DETAILS</div>
				<div class="mt-3 grid grid-cols-1 gap-2 text-sm text-zinc-200">
					<div class="flex items-center justify-between gap-3">
						<span class="text-zinc-400">Cheats</span>
						<span class="font-[var(--dorch-mono)] tracking-wide">{info()?.sv_cheats ? 'on' : 'off'}</span>
					</div>
					<div class="flex items-center justify-between gap-3">
						<span class="text-zinc-400">Monsters</span>
						<span class="font-[var(--dorch-mono)] tracking-wide">{info()?.sv_monsters ? 'on' : 'off'}</span>
					</div>
					<div class="flex items-center justify-between gap-3">
						<span class="text-zinc-400">Fast Monsters</span>
						<span class="font-[var(--dorch-mono)] tracking-wide">{info()?.sv_fastmonsters ? 'on' : 'off'}</span>
					</div>
				</div>
			</div>
		</div>
	</div>
</section>
