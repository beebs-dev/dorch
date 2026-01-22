<svelte:options runes={false} />

<script lang="ts">
	import { onDestroy, onMount, tick } from 'svelte';
	import { resolve } from '$app/paths';

	type JumbotronItem = {
		game_id: string;
		// Back-compat: some APIs provide a single HLS `url`.
		// Newer callers can provide explicit `hls` / `rtc`.
		hls?: string;
		rtc?: string;
		url?: string;

		thumbnail?: string;
		name?: string;
		player_count?: number;
		max_players?: number;
		monster_kill_count?: number;
		monster_total?: number;
	};

	export let items: JumbotronItem[] = [];
	export let visibleCount = 1;

	let mounted = false;
	let activeIndex = 0;
	let switchingSeq = 0;
	let activeItem: JumbotronItem | null = null;
	let activeGameId: string | null = null;
	let activeReady = false;

	// Active stream attachment (only one stream attached at a time).
	type HlsModule = typeof import('hls.js');
	let Hls: HlsModule['default'] | null = null;
	let hls: import('hls.js').default | null = null;
	let rtc: RTCPeerConnection | null = null;

	const videoEls = new Map<string, HTMLVideoElement>();

	function wrapIndex(index: number, len: number): number {
		if (len <= 0) return 0;
		return ((index % len) + len) % len;
	}

	$: activeIndex = wrapIndex(activeIndex, items.length);
	$: activeItem = items.length ? items[activeIndex] : null;
	$: activeGameId = activeItem?.game_id ?? null;
	$: if (mounted) {
		// Whenever we switch active items, show the thumbnail until the stream is actually playing.
		// (Depend on activeGameId so this re-runs when the active slide changes.)
		if (activeGameId) activeReady = false;
		else activeReady = false;
	}

	function rotate(delta: -1 | 1) {
		if (items.length < 2) return;
		activeIndex = wrapIndex(activeIndex + delta, items.length);
	}

	function setActiveByOffset(offset: number) {
		if (!items.length) return;
		activeIndex = wrapIndex(activeIndex + offset, items.length);
	}

	function visibleOffsets(n: number): number[] {
		const clamped = Math.max(0, Math.floor(n));
		const out: number[] = [];
		for (let i = -clamped; i <= clamped; i++) out.push(i);
		return out;
	}

	let offsets: number[] = [];
	let visible: Array<{ offset: number; index: number; item: JumbotronItem; active: boolean }> = [];

	$: offsets = visibleOffsets(visibleCount);
	$: visible = offsets
		.map((offset: number) => {
			const index = items.length ? wrapIndex(activeIndex + offset, items.length) : 0;
			const item = items[index];
			return {
				offset,
				index,
				item,
				active: offset === 0
			};
		})
		.filter(
			(x: {
				offset: number;
				index: number;
				item: JumbotronItem | undefined;
				active: boolean;
			}): x is { offset: number; index: number; item: JumbotronItem; active: boolean } => Boolean(x.item)
		);

	function registerVideo(node: HTMLVideoElement, params: { gameId: string }) {
		videoEls.set(params.gameId, node);
		return {
			destroy() {
				const current = videoEls.get(params.gameId);
				if (current === node) videoEls.delete(params.gameId);
			}
		};
	}

	function rtcSupported(): boolean {
		return typeof (globalThis as any).RTCPeerConnection !== 'undefined';
	}

	function rtcApiUrl(): string {
		return 'https://cdn.gib.gg/rtc/v1/play/';
	}

	function rtcStreamUrl(gameId: string): string {
		return `webrtc://cdn.gib.gg/live/${encodeURIComponent(gameId)}`;
	}

	function destroyRtc(instance: RTCPeerConnection | null) {
		if (!instance) return;
		try {
			instance.close();
		} catch {
			// ignore
		}
	}

	function stopAndResetVideo(video: HTMLVideoElement | null) {
		if (!video) return;
		try {
			video.pause();
		} catch {
			// ignore
		}
		try {
			video.srcObject = null;
		} catch {
			// ignore
		}
		video.removeAttribute('src');
		try {
			video.load();
		} catch {
			// ignore
		}
	}

	function destroyHls(instance: import('hls.js').default | null) {
		if (!instance) return;
		try {
			instance.destroy();
		} catch {
			// ignore
		}
	}

	async function ensureHlsLoaded() {
		if (Hls) return;
		const mod = await import('hls.js');
		Hls = mod.default;
	}

	async function safePlay(video: HTMLVideoElement) {
		try {
			await video.play();
		} catch {
			// Autoplay policies or transient errors; ignore.
		}
	}

	async function attachRtcToVideo(
		item: JumbotronItem,
		video: HTMLVideoElement,
		existing: RTCPeerConnection | null
	): Promise<{ ok: boolean; rtc: RTCPeerConnection | null }> {
		destroyRtc(existing);
		try {
			video.srcObject = null;
		} catch {
			// ignore
		}

		const pc = new RTCPeerConnection();
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
			try {
				stream.addTrack(evt.track);
			} catch {
				// ignore
			}
			try {
				video.srcObject = stream;
			} catch {
				// ignore
			}
		};

		try {
			const offer = await pc.createOffer();
			await pc.setLocalDescription(offer);

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
			if (!offerSdp) {
				destroyRtc(pc);
				return { ok: false, rtc: null };
			}

			const api = rtcApiUrl();
			const streamurl = item.rtc ?? rtcStreamUrl(item.game_id);
			const res = await fetch(api, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ api, streamurl, sdp: offerSdp })
			});
			if (!res.ok) {
				destroyRtc(pc);
				return { ok: false, rtc: null };
			}

			let json: any = null;
			try {
				json = await res.json();
			} catch {
				destroyRtc(pc);
				return { ok: false, rtc: null };
			}

			const answerSdp: string | undefined =
				(typeof json?.sdp === 'string' && json.sdp) ||
				(typeof json?.data?.sdp === 'string' && json.data.sdp) ||
				undefined;
			if (!answerSdp) {
				destroyRtc(pc);
				return { ok: false, rtc: null };
			}

			await pc.setRemoteDescription({ type: 'answer', sdp: answerSdp });
			await safePlay(video);

			const ok = await Promise.race([
				new Promise<boolean>((resolve) => {
					let settled = false;
					const settle = (v: boolean) => {
						if (settled) return;
						settled = true;
						cleanup();
						resolve(v);
					};
					const onPlaying = () => settle(true);
					const onError = () => settle(false);
					const cleanup = () => {
						video.removeEventListener('playing', onPlaying);
						video.removeEventListener('error', onError);
					};
					video.addEventListener('playing', onPlaying);
					video.addEventListener('error', onError);
				}),
				new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 3500))
			]);

			if (!ok) {
				destroyRtc(pc);
				return { ok: false, rtc: null };
			}

			return { ok: true, rtc: pc };
		} catch {
			destroyRtc(pc);
			return { ok: false, rtc: null };
		}
	}

	async function attachStreamToVideo(item: JumbotronItem, video: HTMLVideoElement) {
		const hlsUrl = item.hls ?? item.url;
		if (!hlsUrl) {
			stopAndResetVideo(video);
			destroyHls(hls);
			destroyRtc(rtc);
			hls = null;
			rtc = null;
			return;
		}

		// Reset previous attachments.
		stopAndResetVideo(video);
		destroyHls(hls);
		destroyRtc(rtc);
		hls = null;
		rtc = null;

		video.muted = true;
		video.playsInline = true;
		video.autoplay = true;
		video.loop = true;
		video.preload = 'auto';

		// Prefer RTC when supported.
		if (rtcSupported()) {
			const rtcRes = await attachRtcToVideo(item, video, null);
			if (rtcRes.ok && rtcRes.rtc) {
				rtc = rtcRes.rtc;
				return;
			}
		}

		// Native HLS (Safari) or fallback.
		if (video.canPlayType('application/vnd.apple.mpegurl')) {
			video.src = hlsUrl;
			return;
		}

		await ensureHlsLoaded();
		if (!Hls) {
			video.src = hlsUrl;
			return;
		}

		if (!Hls.isSupported()) {
			video.src = hlsUrl;
			return;
		}

		hls = new Hls({
			enableWorker: true,
			lowLatencyMode: true,
			backBufferLength: 30
		});
		hls.attachMedia(video);
		hls.loadSource(hlsUrl);
	}

	let lastActiveGameId: string | null = null;
	async function activateStream(item: JumbotronItem | null) {
		const seq = ++switchingSeq;
		if (!mounted) return;

		// Stop and detach the previous active stream (if still mounted).
		if (lastActiveGameId) {
			const prevVideo = videoEls.get(lastActiveGameId);
			stopAndResetVideo(prevVideo ?? null);
		}
		destroyHls(hls);
		destroyRtc(rtc);
		hls = null;
		rtc = null;

		lastActiveGameId = item?.game_id ?? null;
		if (!item) return;
		const gameId = item.game_id;

		// Wait until the active element exists in the DOM.
		for (let i = 0; i < 5; i++) {
			if (videoEls.get(gameId)) break;
			await tick();
		}
		const video = videoEls.get(gameId);
		if (!video) return;

		await attachStreamToVideo(item, video);
		if (seq !== switchingSeq) return;
		await safePlay(video);
	}

	$: if (mounted) {
		// Re-attach whenever active item changes.
		if (activeGameId !== lastActiveGameId) {
			void activateStream(activeItem);
		}
	}

	onMount(() => {
		mounted = true;
		// Start on the first item (if any).
		activeIndex = wrapIndex(activeIndex, items.length);
		void activateStream(activeItem);
		return () => {
			mounted = false;
		};
	});

	onDestroy(() => {
		mounted = false;
		destroyHls(hls);
		destroyRtc(rtc);
		hls = null;
		rtc = null;
		// Stop any remaining videos in view.
		for (const v of videoEls.values()) stopAndResetVideo(v);
		videoEls.clear();
	});

	function itemStyle(offset: number) {
		const abs = Math.abs(offset);
		// Translate in element-width percentages so it stays responsive.
		// Since inactive cards are physically smaller, bump the offset step a bit so they still "peek"
		// from behind the active card.
		const step = 88; // percent of element width
		const x = offset * step;
		const z = 30 - abs;
		const dim = offset === 0 ? 1 : 0.72;
		return `--x:${x}%; --z:${z}; --dim:${dim};`;
	}
</script>

<div class="jc-root">

	{#if !items.length}
		<div class="jc-empty grid place-items-center">
			<div class="rounded-xl bg-black/40 px-4 py-3 text-sm text-zinc-200 ring-1 ring-white/10">
				No live streams right now.
			</div>
		</div>
	{:else}
		<div class="jc-stage" aria-roledescription="carousel">
			{#each visible as v (v.item.game_id)}
				<a
					href={resolve(`/servers/${encodeURIComponent(v.item.game_id)}`)}
					class="jc-card"
					data-active={v.active ? 'true' : 'false'}
					data-ready={v.active && activeReady ? 'true' : 'false'}
					style={itemStyle(v.offset)}
					aria-label={v.active
						? `Open server ${v.item.name ?? v.item.game_id}`
						: `Select stream ${v.item.name ?? v.item.game_id}`}
					onclick={(e) => {
						if (!v.active) {
							e.preventDefault();
							setActiveByOffset(v.offset);
						}
					}}
				>
					<div class="jc-media">
						<video
							class="jc-video"
							use:registerVideo={{ gameId: v.item.game_id }}
							onplaying={() => {
								if (v.active && v.item.game_id === activeGameId) activeReady = true;
							}}
							playsinline
							muted
							autoplay
							loop
							preload={v.active ? 'auto' : 'none'}
						></video>
						{#if v.item.thumbnail}
							<img
								src={v.item.thumbnail}
								alt={v.item.name ?? v.item.game_id}
								class="jc-thumb"
								loading="lazy"
							/>
						{:else}
							<div class="jc-thumb jc-thumb-fallback"></div>
						{/if}
						<div class="jc-gloss"></div>
					</div>

					{#if v.active}
						<div class="jc-badge">
							<span class="jc-live-dot"></span>
							LIVE
							<span class="jc-badge-name">{v.item.name ?? v.item.game_id}</span>
						</div>

						<div class="jc-meta">
							{#if v.item.max_players != null}
								<div class="jc-pill jc-pill-sky">
									{v.item.player_count ?? 0} / {v.item.max_players} players
								</div>
							{/if}
							{#if v.item.monster_total != null}
								<div class="jc-pill jc-pill-red">
									{v.item.monster_kill_count ?? 0} / {v.item.monster_total} kills
								</div>
							{/if}
						</div>
					{/if}
				</a>
			{/each}

			{#if items.length > 1}
				<div class="jc-controls" aria-hidden="false">
					<button
						type="button"
						class="jc-nav"
						onclick={() => rotate(-1)}
						aria-label="Previous stream"
						title="Previous"
					>
						<svg viewBox="0 0 20 20" fill="currentColor" class="h-5 w-5" aria-hidden="true">
							<path
								fill-rule="evenodd"
								d="M12.78 15.53a.75.75 0 0 1-1.06 0l-5-5a.75.75 0 0 1 0-1.06l5-5a.75.75 0 1 1 1.06 1.06L8.31 10l4.47 4.47a.75.75 0 0 1 0 1.06z"
								clip-rule="evenodd"
							/>
						</svg>
					</button>
					<button
						type="button"
						class="jc-nav"
						onclick={() => rotate(1)}
						aria-label="Next stream"
						title="Next"
					>
						<svg viewBox="0 0 20 20" fill="currentColor" class="h-5 w-5" aria-hidden="true">
							<path
								fill-rule="evenodd"
								d="M7.22 4.47a.75.75 0 0 1 1.06 0l5 5a.75.75 0 0 1 0 1.06l-5 5a.75.75 0 0 1-1.06-1.06L11.69 10 7.22 5.53a.75.75 0 0 1 0-1.06z"
								clip-rule="evenodd"
							/>
						</svg>
					</button>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.jc-root {
		position: relative;
		isolation: isolate;
		overflow: visible;
		/* Card sizing (active vs inactive) */
		--jc-active-w: min(860px, 40vw);
		--jc-active-h: calc(var(--jc-active-w) * 9 / 16);
		--jc-inactive-w: calc(var(--jc-active-w) * 0.75);
		--jc-inactive-h: calc(var(--jc-active-h) * 0.75);
		min-height: clamp(220px, 26vw, 360px);
	}

	.jc-empty {
		position: relative;
		min-height: clamp(220px, 26vw, 360px);
	}

	.jc-stage {
		position: relative;
		/* IMPORTANT: stage must be tall enough for the active card, or everything gets clipped */
		height: clamp(220px, var(--jc-active-h), 520px);
		width: 100%;
		overflow: visible;
	}

	.jc-card {
		display: block;
		position: absolute;
		left: 50%;
		top: 50%;
		--x: 0%;
		--z: 1;
		--dim: 1;
		transform: translate(-50%, -50%) translateX(var(--x, 0%));
		z-index: var(--z);
		/* Inactive thumbnails are 30% smaller than the active item. */
		width: var(--jc-inactive-w);
		height: var(--jc-inactive-h);
		border-radius: 16px;
		overflow: hidden;
		background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
		box-shadow:
			0 25px 70px rgba(0, 0, 0, 0.55),
			0 0 0 1px rgba(255, 255, 255, 0.08) inset;
		transition:
			transform 520ms cubic-bezier(0.22, 1, 0.36, 1),
			filter 520ms cubic-bezier(0.22, 1, 0.36, 1);
		will-change: transform;
		cursor: pointer;
		filter: brightness(var(--dim));
	}

	.jc-card[data-active='true'] {
		width: var(--jc-active-w);
		height: var(--jc-active-h);
	}

	.jc-card:focus-visible {
		outline: 2px solid rgba(228, 228, 231, 0.65);
		outline-offset: 3px;
	}

	.jc-media {
		position: absolute;
		inset: 0;
	}

	.jc-video,
	.jc-thumb {
		position: absolute;
		inset: 0;
		height: 100%;
		width: 100%;
		object-fit: cover;
	}

	/* Only the centered slide *can* show the video, and only after it's playing. */
	.jc-card .jc-video {
		opacity: 0;
		transition: opacity 220ms ease;
	}
	.jc-card .jc-thumb {
		opacity: 1;
		transition: opacity 220ms ease;
	}

	.jc-card[data-active='true'][data-ready='true'] .jc-video {
		opacity: 1;
	}
	.jc-card[data-active='true'][data-ready='true'] .jc-thumb {
		opacity: 0;
	}

	.jc-thumb-fallback {
		background:
			radial-gradient(circle at 25% 20%, rgba(244, 63, 94, 0.18), transparent 55%),
			radial-gradient(circle at 80% 60%, rgba(168, 85, 247, 0.18), transparent 55%),
			linear-gradient(to bottom, rgba(24, 24, 27, 0.6), rgba(0, 0, 0, 0.8));
	}

	.jc-gloss {
		position: absolute;
		inset: 0;
		pointer-events: none;
		background: linear-gradient(to bottom, rgba(0, 0, 0, 0), rgba(0, 0, 0, 0.28));
	}

	.jc-badge {
		position: absolute;
		left: 16px;
		top: 14px;
		z-index: 5;
		display: inline-flex;
		min-width: 0;
		align-items: center;
		gap: 8px;
		border-radius: 999px;
		background: rgba(0, 0, 0, 0.45);
		padding: 6px 10px;
		font-family: var(--dorch-mono);
		font-size: 12px;
		letter-spacing: 0.06em;
		color: rgba(244, 244, 245, 0.92);
		backdrop-filter: blur(8px);
		box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.12) inset;
		max-width: calc(100% - 32px);
	}

	.jc-live-dot {
		height: 8px;
		width: 8px;
		border-radius: 999px;
		background: rgb(239, 68, 68);
		box-shadow: 0 0 18px rgba(239, 68, 68, 0.65);
		flex: none;
	}

	.jc-badge-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: rgba(228, 228, 231, 0.92);
	}

	.jc-meta {
		position: absolute;
		left: 16px;
		bottom: 14px;
		z-index: 5;
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.jc-pill {
		display: inline-flex;
		align-items: center;
		border-radius: 999px;
		padding: 6px 10px;
		font-size: 12px;
		font-family: var(--dorch-mono);
		letter-spacing: 0.04em;
		backdrop-filter: blur(8px);
		box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.12) inset;
	}

	.jc-pill-sky {
		background: rgba(8, 47, 73, 0.35);
		color: rgba(224, 242, 254, 0.92);
	}

	.jc-pill-red {
		background: rgba(69, 10, 10, 0.45);
		color: rgba(254, 226, 226, 0.92);
	}

	.jc-controls {
		position: absolute;
		inset: 0;
		z-index: 50;
		pointer-events: none;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 12px;
	}

	.jc-nav {
		pointer-events: auto;
		display: grid;
		place-items: center;
		height: 40px;
		width: 40px;
		border-radius: 999px;
		background: rgba(0, 0, 0, 0.45);
		color: rgba(244, 244, 245, 0.92);
		box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.15) inset;
		backdrop-filter: blur(8px);
		transition: background 180ms ease;
	}

	.jc-nav:hover {
		background: rgba(0, 0, 0, 0.6);
	}

	@media (max-width: 640px) {
		.jc-card {
			width: 92vw;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.jc-card {
			transition: none;
		}
		.jc-card .jc-video,
		.jc-card .jc-thumb {
			transition: none;
		}
	}
</style>
