<script lang="ts">
	import type { PageData } from './$types';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import DorchPlayButton from '$lib/components/DorchPlayButton.svelte';
	import { wadLabel } from '$lib/utils/format';
	import { showToast } from '$lib/stores/toast';

	let { data }: { data: PageData } = $props();

	const game = $derived(() => data.game);
	const spec = $derived(() => data.game.spec);
	const status = $derived(() => data.game.status);
	const info = $derived(() => data.game.info);
	const currentMap = $derived(() => data.currentMap);
	const currentMapWadId = $derived(() => data.currentMapWadId);

	const pageTitle = $derived(() => `${info()?.name ?? spec().name ?? data.gameId} - ɢɪʙ.ɢɢ`);
	const videoSrcHLS = $derived(
		() => `https://cdn.gib.gg/live/${encodeURIComponent(data.gameId)}.m3u8`
	);
	const videoSrcRTCStream = $derived(
		() => `webrtc://cdn.gib.gg/live/${encodeURIComponent(data.gameId)}`
	);
	const videoSrcRTCApi = $derived(() => `https://cdn.gib.gg/rtc/v1/play/`);

	let identity = $state(randomIdent());
	let showGameId = $state(false);

	async function copyToClipboard(text: string) {
		try {
			await navigator.clipboard.writeText(text);
		} catch {
			const ta = document.createElement('textarea');
			ta.value = text;
			ta.setAttribute('readonly', '');
			ta.style.position = 'fixed';
			ta.style.left = '-9999px';
			document.body.appendChild(ta);
			ta.select();
			document.execCommand('copy');
			document.body.removeChild(ta);
		}
		showToast('Copied to clipboard');
	}

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
		return `/play/?g=${encodeURIComponent(data.gameId)}&identity=${encodeURIComponent(identity)}`;
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

	function statusDotColor(phase: string | undefined): string {
		switch (phase) {
			case 'Active':
				return '#34d399'; // emerald-400
			case 'Starting':
				return '#fbbf24'; // amber-400
			case 'Pending':
				return '#60a5fa'; // blue-400
			case 'Terminating':
				return '#fb7185'; // rose-400
			case 'Error':
				return '#f87171'; // red-400
			default:
				return '#a1a1aa'; // zinc-400
		}
	}

	function statusText(phase: string | undefined): string {
		return phase && phase.length ? phase : 'Unknown';
	}

	let videoEl = $state<HTMLVideoElement | null>(null);
	let hlsInstance: { destroy(): void } | null = null;
	let hlsError = $state<string | null>(null);
	let usingRtc = $state(false);
	let rtcPc: RTCPeerConnection | null = null;

	function rtcSupported(): boolean {
		return typeof (globalThis as any).RTCPeerConnection !== 'undefined';
	}

	function destroyRtc() {
		try {
			rtcPc?.close();
		} catch {
			// ignore
		} finally {
			rtcPc = null;
		}
	}

	function destroyHls() {
		try {
			hlsInstance?.destroy();
		} finally {
			hlsInstance = null;
		}
	}

	async function tryPlayRtc(
		el: HTMLVideoElement,
		streamUrl: string,
		apiUrl: string
	): Promise<boolean> {
		// SRS WebRTC play API: POST offer SDP + streamurl to `/rtc/v1/play/`, receive answer SDP.
		usingRtc = false;
		try {
			el.srcObject = null;
		} catch {
			// ignore
		}
		el.removeAttribute('src');
		destroyHls();
		destroyRtc();
		hlsError = null;

		const pc = new RTCPeerConnection();
		rtcPc = pc;

		const stream = new MediaStream();
		pc.addTransceiver('video', { direction: 'recvonly' });
		pc.addTransceiver('audio', { direction: 'recvonly' });
		pc.ontrack = (evt) => {
			for (const track of evt.streams?.[0]?.getTracks?.() ?? []) {
				try {
					stream.addTrack(track);
				} catch {
					// ignore
				}
			}
			// Some browsers do not populate evt.streams.
			try {
				stream.addTrack(evt.track);
			} catch {
				// ignore
			}
			try {
				el.srcObject = stream;
			} catch {
				// ignore
			}
		};

		const onPlaying = new Promise<boolean>((resolve) => {
			let settled = false;
			const settle = (ok: boolean) => {
				if (settled) return;
				settled = true;
				cleanup();
				resolve(ok);
			};
			const cleanup = () => {
				el.removeEventListener('playing', handlePlaying);
				el.removeEventListener('error', handleError);
			};
			const handlePlaying = () => settle(true);
			const handleError = () => settle(false);
			el.addEventListener('playing', handlePlaying);
			el.addEventListener('error', handleError);
		});

		try {
			const offer = await pc.createOffer();
			await pc.setLocalDescription(offer);

			// Wait for ICE gathering so the SDP includes candidates (SRS expects this).
			await new Promise<void>((resolve) => {
				if (pc.iceGatheringState === 'complete') return resolve();
				const onStateChange = () => {
					if (pc.iceGatheringState === 'complete') {
						pc.removeEventListener('icegatheringstatechange', onStateChange);
						resolve();
					}
				};
				pc.addEventListener('icegatheringstatechange', onStateChange);
				setTimeout(() => {
					pc.removeEventListener('icegatheringstatechange', onStateChange);
					resolve();
				}, 1500);
			});

			const offerSdp = pc.localDescription?.sdp;
			if (!offerSdp) return false;

			const res = await fetch(apiUrl, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ api: apiUrl, streamurl: streamUrl, sdp: offerSdp })
			});
			if (!res.ok) return false;

			let json: any = null;
			try {
				json = await res.json();
			} catch {
				return false;
			}

			const answerSdp: string | undefined =
				(typeof json?.sdp === 'string' && json.sdp) ||
				(typeof json?.data?.sdp === 'string' && json.data.sdp) ||
				undefined;
			if (!answerSdp) return false;

			await pc.setRemoteDescription({ type: 'answer', sdp: answerSdp });

			// Try to start playback (muted is already set on the element).
			try {
				await el.play();
			} catch {
				// ignore
			}
		} catch {
			return false;
		}

		const timeoutMs = 4000;
		const ok = await Promise.race([
			onPlaying,
			new Promise<boolean>((resolve) => setTimeout(() => resolve(false), timeoutMs))
		]);

		if (ok) {
			usingRtc = true;
			console.info('[LIVE VIEW] using RTC');
			return true;
		}

		// Ensure we don't leak connections if we ended up falling back.
		destroyRtc();
		return ok;
	}

	async function setupHls(el: HTMLVideoElement, src: string, destroyedRef: () => boolean) {
		usingRtc = false;
		destroyHls();
		destroyRtc();
		hlsError = null;
		console.info('[LIVE VIEW] using HLS');

		// Native HLS support (Safari, iOS).
		if (el.canPlayType('application/vnd.apple.mpegurl')) {
			el.src = src;
			return;
		}

		const mod = await import('hls.js');
		const Hls = mod.default;
		if (destroyedRef()) return;

		if (!Hls.isSupported()) {
			// Some browsers can still play HLS without canPlayType matching.
			el.src = src;
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
		hls.attachMedia(el);
	}

	onMount(() => {
		let destroyed = false;

		(async () => {
			if (!videoEl) return;

			// Prefer RTC if the browser has WebRTC APIs.
			if (rtcSupported()) {
				const ok = await tryPlayRtc(videoEl, videoSrcRTCStream(), videoSrcRTCApi());
				if (destroyed) return;
				if (ok) return;
			}

			await setupHls(videoEl, videoSrcHLS(), () => destroyed);
		})();

		return () => {
			destroyed = true;
			destroyHls();
			destroyRtc();
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
			<nav aria-label="Breadcrumb" class="min-w-0">
				<ol class="flex min-w-0 items-baseline gap-2">
					<li class="shrink-0">
						<a
							href={resolve('/')}
							class="text-xs font-[var(--dorch-mono)] font-medium tracking-wide text-zinc-400 hover:text-zinc-200 focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-500"
						>
							SERVERS
						</a>
					</li>
					<li aria-hidden="true" class="shrink-0 text-xs text-zinc-600">
						/
					</li>
					<li class="min-w-0" aria-current="page">
						<h1 class="truncate text-2xl font-semibold tracking-tight text-zinc-100">
							{info()?.name ?? spec().name ?? 'Game'}
						</h1>
					</li>
				</ol>
			</nav>
		</div>
		<div class="flex items-center gap-2">
			<a
				href={resolve('/')}
				class="rounded-md bg-zinc-900 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
			>
				Back to servers
			</a>
		</div>
	</header>

	<div class="mt-5 grid grid-cols-1 gap-6 lg:grid-cols-2">
		<!-- Left: basic server info + JOIN -->
		<div class="space-y-4">
			<div class="rounded-xl bg-zinc-950 p-4 ring-1 ring-red-950/60 ring-inset">
				<div class="flex items-start justify-between gap-4">
					<div class="min-w-0">
						<div class="mt-1 truncate text-lg font-semibold text-zinc-100">
							{info()?.name ?? spec().name ?? '(unknown)'}
						</div>
						<div class="mt-1 text-sm {difficultyColor(info()?.skill ?? spec().skill ?? undefined)}">
							{#if info() || spec().skill != null}
								{difficultyLabel(info()?.skill ?? spec().skill ?? undefined)}
							{:else}
								<span class="skeleton inline-block h-4 w-44 rounded-md align-middle" aria-label="Loading difficulty"></span>
							{/if}
						</div>
					</div>
					<div class="shrink-0">
						{#if status() === 'Active'}
							<DorchPlayButton
								href={joinUrl()}
								label="JOIN"
								ariaLabel="Join"
								className="inline-flex rounded-xl px-8 py-4 text-xl"
							/>
						{:else}
							<div
								class="inline-flex cursor-not-allowed items-center justify-center rounded-xl px-8 py-4 text-xl text-zinc-300 ring-1 ring-red-950/60 ring-inset bg-zinc-900/50"
								aria-label="Join (starting)"
								title="Server is still starting"
							>
							{#if status() === 'Active'}
								J O I N
							{:else}
								{statusText(status())}
							{/if}
							</div>
						{/if}
						<div
							class="mt-2 text-center text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-500"
						>
							identity: {identity}
						</div>
					</div>
				</div>

				<div class="mt-3 flex w-full flex-wrap items-center justify-start gap-x-8 gap-y-2 text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">
					<div class="flex items-center gap-2">
						<span
							class="inline-block h-2.5 w-2.5 rounded-full ring-2 ring-white/10"
							style={`background-color: ${statusDotColor(status())}`}
							aria-label="Status"
							title={`Status: ${statusText(status())}`}
						></span>
						<span>Status: {statusText(status())}</span>
					</div>
					<div class="flex items-center gap-2">
						<span>GAME ID:</span>
						{#if showGameId}
							<button
								type="button"
								class="cursor-pointer font-[var(--dorch-mono)] text-zinc-200"
								onclick={() => copyToClipboard(data.gameId)}
							>
								{data.gameId}
							</button>
						{:else}
							<button
								type="button"
								class="text-zinc-400 underline hover:text-zinc-200"
								onclick={() => (showGameId = true)}
							>
								Show
							</button>
						{/if}
					</div>
				</div>

				<div class="mt-4 grid grid-cols-2 gap-3">
					<div class="rounded-lg bg-zinc-900/40 p-3 ring-1 ring-red-950/40 ring-inset">
						<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">PLAYERS</div>
						<div class="mt-1 text-lg font-[var(--dorch-mono)] tracking-wide text-zinc-100">
							{#if status() == "Active" && info()?.player_count != null && info()?.max_players != null}
								{info()!.player_count}/{info()!.max_players}
							{:else}
								<span class="skeleton inline-block h-5 w-24 rounded-md align-middle" aria-label="Loading player counts"></span>
							{/if}
						</div>
					</div>
					<div class="rounded-lg bg-zinc-900/40 p-3 ring-1 ring-red-950/40 ring-inset">
						<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">KILLS</div>
						<div class="mt-1 text-lg font-[var(--dorch-mono)] tracking-wide text-zinc-100">
							{#if status() == "Active" && info()?.monster_kill_count != null && info()?.monster_count != null}
								{info()!.monster_kill_count}/{info()!.monster_count}
							{:else}
								<span class="skeleton inline-block h-5 w-24 rounded-md align-middle" aria-label="Loading kill counts"></span>
							{/if}
						</div>
					</div>
				</div>

				<div class="mt-4 rounded-lg bg-zinc-900/40 p-3 ring-1 ring-red-950/40 ring-inset">
					<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">
						CURRENT MAP
					</div>
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
							{#if info()}
								<div class="text-sm text-zinc-400">Unknown</div>
							{:else}
								<span class="skeleton inline-block h-5 w-28 rounded-md" aria-label="Loading current map"></span>
							{/if}
						{/if}
					</div>
				</div>
			</div>

			<div class="rounded-xl bg-zinc-950 p-4 ring-1 ring-red-950/60 ring-inset">
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
							{#if wad.maps.length}
								<details class="mt-2">
									<summary
										class="cursor-pointer text-xs text-zinc-400 select-none hover:text-zinc-200"
									>
										Maps ({wad.maps.length})
									</summary>
									<div class="mt-2 flex flex-wrap gap-2">
										{#each wad.maps as m (m.map)}
											<a
												href={mapHref(wad.id, m.map)}
												class={`rounded-md px-2 py-1 text-xs font-[var(--dorch-mono)] tracking-wide ring-1 ring-inset hover:bg-zinc-800/50 ${
													isCurrentMap(m.map)
														? 'bg-red-950/30 text-zinc-100 ring-red-900/60'
														: 'bg-zinc-950 text-zinc-200 ring-red-950/40'
												}`}
												title={m.title && m.title !== m.map ? m.title : undefined}
											>
												{#if m.title}
													<span>{m.map}</span>
													<span class="ml-1 text-[0.7rem] font-normal text-zinc-400">({m.title})</span>
												{:else}
													{m.map}
												{/if}
											</a>
										{/each}
									</div>
								</details>
							{:else}
								<div class="mt-2 text-xs text-zinc-500">
									No maps available (or wadinfo unavailable).
								</div>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		</div>

		<!-- Right: hero video -->
		<div class="space-y-4">
			<div class="overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset">
				<div class="flex items-center justify-between bg-red-950/25 px-4 py-3">
					<div
						class="flex items-center gap-2 text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-200"
					>
						<span
							class="inline-block h-2.5 w-2.5 rounded-full bg-red-500 ring-2 ring-red-200/20"
							aria-label="Recording"
							title="Recording"
						></span>
						<span>LIVE VIEW</span>
					</div>
					<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">
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
						<div
							class="absolute bottom-3 left-3 rounded-md bg-zinc-950/80 px-3 py-2 text-xs text-zinc-200 ring-1 ring-red-950/60 ring-inset"
						>
							HLS error: {hlsError}
						</div>
					{/if}
				</div>
			</div>

			<div class="rounded-xl bg-zinc-950 p-4 ring-1 ring-red-950/60 ring-inset">
				<div class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-400">
					SERVER DETAILS
				</div>
				<div class="mt-3 grid grid-cols-1 gap-2 text-sm text-zinc-200">
					<div class="flex items-center justify-between gap-3">
						<span class="text-zinc-400">Cheats</span>
						{#if info()}
							<span class="font-[var(--dorch-mono)] tracking-wide"
								>{info()!.sv_cheats ? 'on' : 'off'}</span
							>
						{:else}
							<span class="skeleton inline-block h-4 w-12 rounded-md" aria-label="Loading cheats"></span>
						{/if}
					</div>
					<div class="flex items-center justify-between gap-3">
						<span class="text-zinc-400">Monsters</span>
						{#if info()}
							<span class="font-[var(--dorch-mono)] tracking-wide"
								>{info()!.sv_monsters ? 'on' : 'off'}</span
							>
						{:else}
							<span class="skeleton inline-block h-4 w-12 rounded-md" aria-label="Loading monsters"></span>
						{/if}
					</div>
					<div class="flex items-center justify-between gap-3">
						<span class="text-zinc-400">Fast Monsters</span>
						{#if info()}
							<span class="font-[var(--dorch-mono)] tracking-wide"
								>{info()!.sv_fastmonsters ? 'on' : 'off'}</span
							>
						{:else}
							<span class="skeleton inline-block h-4 w-12 rounded-md" aria-label="Loading fast monsters"></span>
						{/if}
					</div>
				</div>
			</div>
		</div>
	</div>
</section>

<style>
	.skeleton {
		background: linear-gradient(
			90deg,
			rgba(63, 63, 70, 0.25) 0%,
			rgba(63, 63, 70, 0.5) 40%,
			rgba(63, 63, 70, 0.25) 80%
		);
		background-size: 200% 100%;
		animation: dorch-skeleton 1.25s ease-in-out infinite;
	}

	@keyframes dorch-skeleton {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.skeleton {
			animation: none;
			background-position: 0 0;
		}
	}
</style>
