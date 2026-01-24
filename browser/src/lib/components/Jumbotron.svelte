<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';

	type JumbotronItem = {
		game_id: string;
		// Back-compat: some APIs provide a single HLS `url`.
		// Newer callers can provide explicit `hls` / `rtc`.
		hls?: string;
		rtc?: string;
		url?: string;
		name?: string;
		player_count?: number;
		max_players?: number;
		monster_kill_count?: number;
		monster_total?: number;
	};

	export let items: JumbotronItem[] = [];
	export let intervalMs = 5000;
	// If false, the jumbotron will not automatically rotate streams.
	// Navigation is manual via the next/prev buttons.
	export let autoAdvance = false;

	type TransitionKind = 'fade' | 'zoom' | 'slide-left' | 'slide-right' | 'tilt' | 'wipe' | 'glitch';

	type Slot = 'a' | 'b';

	let mounted = false;
	let current: JumbotronItem | null = null;
	let next: JumbotronItem | null = null;
	let activeSlot: Slot = 'a';
	let currentTransition: TransitionKind = 'fade';
	let switching = false;
	let statusText = '';
	let switchSeq = 0;

	let videoA: HTMLVideoElement | null = null;
	let videoB: HTMLVideoElement | null = null;

	type HlsModule = typeof import('hls.js');
	let Hls: HlsModule['default'] | null = null;
	let hlsA: import('hls.js').default | null = null;
	let hlsB: import('hls.js').default | null = null;
	let rtcA: RTCPeerConnection | null = null;
	let rtcB: RTCPeerConnection | null = null;

	let switchTimer: number | null = null;
	let currentIndex = -1;
	let nextIndex = -1;

	let lastItemsKey = '';
	$: {
		const key = items.map((i) => i.game_id).join('|');
		if (mounted && key !== lastItemsKey) {
			lastItemsKey = key;
			void start();
		}
	}

	function randInt(maxExclusive: number): number {
		return Math.floor(Math.random() * maxExclusive);
	}

	function pickDifferentRandom(
		list: JumbotronItem[],
		avoidGameId?: string | null
	): JumbotronItem | null {
		if (!list.length) return null;
		if (list.length === 1) return list[0];

		for (let tries = 0; tries < 8; tries++) {
			const candidate = list[randInt(list.length)];
			if (!avoidGameId || candidate.game_id !== avoidGameId) return candidate;
		}

		return list.find((x) => x.game_id !== avoidGameId) ?? list[0];
	}

	function wrapIndex(index: number, len: number): number {
		if (len <= 0) return -1;
		return ((index % len) + len) % len;
	}

	function itemAt(index: number): JumbotronItem | null {
		if (!items.length) return null;
		const i = wrapIndex(index, items.length);
		return i >= 0 ? items[i] : null;
	}

	function pickTransition(): TransitionKind {
		const kinds: TransitionKind[] = [
			'fade',
			'zoom',
			'slide-left',
			'slide-right',
			'tilt',
			'wipe',
			'glitch'
		];
		return kinds[randInt(kinds.length)];
	}

	function activeVideo(): HTMLVideoElement | null {
		return activeSlot === 'a' ? videoA : videoB;
	}

	function standbyVideo(): HTMLVideoElement | null {
		return activeSlot === 'a' ? videoB : videoA;
	}

	function activeHls(): import('hls.js').default | null {
		return activeSlot === 'a' ? hlsA : hlsB;
	}

	function standbyHls(): import('hls.js').default | null {
		return activeSlot === 'a' ? hlsB : hlsA;
	}

	function setStandbyHls(instance: import('hls.js').default | null) {
		if (activeSlot === 'a') hlsB = instance;
		else hlsA = instance;
	}

	function activeRtc(): RTCPeerConnection | null {
		return activeSlot === 'a' ? rtcA : rtcB;
	}

	function standbyRtc(): RTCPeerConnection | null {
		return activeSlot === 'a' ? rtcB : rtcA;
	}

	function setStandbyRtc(instance: RTCPeerConnection | null) {
		if (activeSlot === 'a') rtcB = instance;
		else rtcA = instance;
	}

	function destroyRtc(instance: RTCPeerConnection | null) {
		if (!instance) return;
		try {
			instance.close();
		} catch {
			// ignore
		}
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

	async function attachRtcToVideo(
		item: JumbotronItem,
		video: HTMLVideoElement,
		existing: RTCPeerConnection | null,
		opts?: { log?: boolean }
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
			// Prefer evt.streams if present; always also add evt.track.
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

			// Try to start playback and give it a moment.
			try {
				await video.play();
			} catch {
				// ignore
			}

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

			if (opts?.log) console.info('[Jumbotron] using RTC');
			return { ok: true, rtc: pc };
		} catch {
			destroyRtc(pc);
			return { ok: false, rtc: null };
		}
	}

	async function attachStreamToVideo(
		item: JumbotronItem,
		video: HTMLVideoElement,
		existingHls: import('hls.js').default | null,
		existingRtc: RTCPeerConnection | null,
		opts?: { log?: boolean }
	): Promise<{ hls: import('hls.js').default | null; rtc: RTCPeerConnection | null }> {
		const hlsUrl = item.hls ?? item.url;
		if (!hlsUrl) {
			destroyHls(existingHls);
			destroyRtc(existingRtc);
			return { hls: null, rtc: null };
		}

		// Prefer RTC when supported.
		if (rtcSupported()) {
			const rtcRes = await attachRtcToVideo(item, video, existingRtc, opts);
			if (rtcRes.ok && rtcRes.rtc) {
				destroyHls(existingHls);
				return { hls: null, rtc: rtcRes.rtc };
			}
		}

		// Reset previous playback / attachment.
		destroyRtc(existingRtc);
		destroyHls(existingHls);
		if (opts?.log) console.info('[Jumbotron] using HLS');

		// Some browsers support native HLS playback (Safari). If so, keep it simple.
		if (video.canPlayType('application/vnd.apple.mpegurl')) {
			video.src = hlsUrl;
			return { hls: null as import('hls.js').default | null, rtc: null };
		}

		await ensureHlsLoaded();
		if (!Hls) return { hls: null, rtc: null };

		if (!Hls.isSupported()) {
			// Last-ditch fallback.
			video.src = hlsUrl;
			return { hls: null, rtc: null };
		}

		const hls = new Hls({
			enableWorker: true,
			lowLatencyMode: true,
			backBufferLength: 30
		});
		hls.attachMedia(video);
		hls.loadSource(hlsUrl);
		return { hls, rtc: null };
	}

	async function waitForPreload(video: HTMLVideoElement, timeoutMs = 2500): Promise<void> {
		// We want the next stream to be playing/buffered before switching.
		// Resolve when we have *some* decoded data.
		const start = Date.now();

		while (Date.now() - start < timeoutMs) {
			// readyState >= 2 means HAVE_CURRENT_DATA.
			if (video.readyState >= 2) return;
			await new Promise((r) => setTimeout(r, 120));
		}
	}

	async function safePlay(video: HTMLVideoElement) {
		try {
			await video.play();
		} catch {
			// Autoplay policies or transient errors; ignore.
		}
	}

	async function primeNextStream(item: JumbotronItem | null): Promise<boolean> {
		if (!mounted) return false;
		if (!item) return false;

		const video = standbyVideo();
		if (!video) return false;

		stopAndResetVideo(video);

		const { hls, rtc } = await attachStreamToVideo(item, video, standbyHls(), standbyRtc(), {
			log: false
		});
		setStandbyHls(hls);
		setStandbyRtc(rtc);

		video.muted = true;
		video.playsInline = true;
		video.autoplay = true;
		video.loop = true;
		video.preload = 'auto';

		await safePlay(video);
		await waitForPreload(video, 2500);
		if (video.error) {
			statusText = '';
			return false;
		}

		statusText = '';
		return true;
	}

	function setActiveVisualState() {
		const aActive = activeSlot === 'a';
		if (videoA) {
			videoA.dataset.active = aActive ? 'true' : 'false';
			videoA.dataset.kind = currentTransition;
		}
		if (videoB) {
			videoB.dataset.active = !aActive ? 'true' : 'false';
			videoB.dataset.kind = currentTransition;
		}
	}

	function armTimer() {
		clearTimer();
		if (!items.length) return;
		if (!autoAdvance) return;
		switchTimer = window.setInterval(() => {
			void switchNow();
		}, intervalMs);
	}

	async function switchNow(opts: { transition?: TransitionKind } = {}) {
		const seq = ++switchSeq;
		if (switching) return;
		if (!current || !next) return;
		if (!mounted) return;

		switching = true;
		try {
			currentTransition = opts.transition ?? pickTransition();
			setActiveVisualState();

			// If the standby stream isn't ready, try to recover by preloading another.
			const standby = standbyVideo();
			if (standby) {
				await waitForPreload(standby, 1800);
				if (seq !== switchSeq) return;
				if (standby.readyState < 2 || standby.error) {
					let recovered = false;
					for (let tries = 0; tries < 3; tries++) {
						const alternative = pickDifferentRandom(items, current?.game_id);
						if (!alternative || alternative.game_id === next.game_id) continue;
						next = alternative;
						recovered = await primeNextStream(next);
						if (seq !== switchSeq) return;
						if (recovered) break;
					}
					if (!recovered) {
						// Don't switch to an unbuffered stream.
						return;
					}
				}
			}

			// Swap active slot.
			activeSlot = activeSlot === 'a' ? 'b' : 'a';
			current = next;
			currentIndex = nextIndex >= 0 ? wrapIndex(nextIndex, items.length) : currentIndex;
			nextIndex = wrapIndex(currentIndex + 1, items.length);
			next = itemAt(nextIndex);

			// Ensure the newly active video is audible (but keep it fairly subtle for UX).
			const active = activeVideo();
			if (active) {
				active.muted = true;
				await safePlay(active);
				if (seq !== switchSeq) return;
			}

			setActiveVisualState();

			// Preload the next candidate immediately (gives us >1–2s headroom).
			await primeNextStream(next);
			if (seq !== switchSeq) return;

			// Let the CSS animation run.
			await new Promise((r) => setTimeout(r, 900));
		} finally {
			if (seq === switchSeq) switching = false;
		}
	}

	function clearTimer() {
		if (switchTimer != null) {
			clearInterval(switchTimer);
			switchTimer = null;
		}
	}

	async function start() {
		clearTimer();
		if (!items.length) return;

		currentIndex = randInt(items.length);
		nextIndex = wrapIndex(currentIndex + 1, items.length);
		current = itemAt(currentIndex);
		next = itemAt(nextIndex);

		// Load current into active slot.
		const active = activeVideo();
		if (active && current) {
			statusText = 'Loading stream…';
			stopAndResetVideo(active);
			const { hls, rtc } = await attachStreamToVideo(current, active, activeHls(), activeRtc(), {
				log: true
			});
			if (activeSlot === 'a') {
				hlsA = hls;
				rtcA = rtc;
			} else {
				hlsB = hls;
				rtcB = rtc;
			}

			active.muted = true;
			active.playsInline = true;
			active.autoplay = true;
			active.loop = true;
			active.preload = 'auto';

			await safePlay(active);
			await waitForPreload(active, 2500);
			statusText = '';
		}

		setActiveVisualState();

		// Preload next right away so transitions are hitch-free.
		await primeNextStream(next);

		armTimer();
	}

	async function step(delta: -1 | 1) {
		if (items.length < 2) return;
		if (!mounted) return;

		clearTimer();
		const seq = ++switchSeq;

		// Cancel any in-flight auto switch and swap immediately.
		switching = false;
		currentTransition = delta === 1 ? 'slide-left' : 'slide-right';
		currentIndex = wrapIndex((currentIndex >= 0 ? currentIndex : 0) + delta, items.length);
		nextIndex = wrapIndex(currentIndex + 1, items.length);
		current = itemAt(currentIndex);
		next = itemAt(nextIndex);

		activeSlot = activeSlot === 'a' ? 'b' : 'a';
		setActiveVisualState();

		const active = activeVideo();
		if (active && current) {
			statusText = 'Loading stream…';
			stopAndResetVideo(active);
			const { hls, rtc } = await attachStreamToVideo(current, active, activeHls(), activeRtc(), {
				log: true
			});
			if (seq !== switchSeq) {
				destroyHls(hls);
				destroyRtc(rtc);
				return;
			}
			if (activeSlot === 'a') {
				hlsA = hls;
				rtcA = rtc;
			} else {
				hlsB = hls;
				rtcB = rtc;
			}

			active.muted = true;
			active.playsInline = true;
			active.autoplay = true;
			active.loop = true;
			active.preload = 'auto';

			void safePlay(active);
			void (async () => {
				await waitForPreload(active, 2500);
				if (seq !== switchSeq) return;
				statusText = '';
			})();
		}

		// Preload the next stream in the background; don't block responsiveness.
		if (next) void primeNextStream(next);

		armTimer();
	}

	onMount(() => {
		mounted = true;
		void start();

		return () => {
			mounted = false;
			clearTimer();

			destroyHls(hlsA);
			destroyHls(hlsB);
			hlsA = null;
			hlsB = null;

			destroyRtc(rtcA);
			destroyRtc(rtcB);
			rtcA = null;
			rtcB = null;

			stopAndResetVideo(videoA);
			stopAndResetVideo(videoB);
		};
	});

	onDestroy(() => {
		mounted = false;
	});
</script>

<div class="relative overflow-hidden rounded-2xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset">
	<div
		class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_20%_15%,rgba(244,63,94,0.18),transparent_55%),radial-gradient(circle_at_80%_65%,rgba(168,85,247,0.18),transparent_55%),linear-gradient(to_bottom,rgba(0,0,0,0.0),rgba(0,0,0,0.35))]"
	></div>

	<a
		href={resolve(current ? `/servers/${encodeURIComponent(current.game_id)}` : '/servers')}
		class="group block"
		aria-label={current ? `Open server ${current.name ?? current.game_id}` : 'Jumbotron'}
	>
		<div class="relative aspect-[16/6] w-full">
			<video
				bind:this={videoA}
				class="jv absolute inset-0 h-full w-full object-cover"
				data-active="true"
				data-kind="fade"
				playsinline
				muted
				autoplay
				loop
				preload="auto"
			></video>
			<video
				bind:this={videoB}
				class="jv absolute inset-0 h-full w-full object-cover"
				data-active="false"
				data-kind="fade"
				playsinline
				muted
				autoplay
				loop
				preload="auto"
			></video>

			<div
				class="pointer-events-none absolute inset-0 z-10 ring-1 ring-white/10 ring-inset group-hover:ring-white/15"
			></div>

			<div class="pointer-events-none absolute top-0 left-0 z-20 p-4">
				<div
					class="text-m inline-flex min-w-0 items-center gap-2 rounded-full bg-black/45 px-3 py-1 font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-white/10 backdrop-blur"
				>
					<span
						class="h-2 w-2 shrink-0 rounded-full bg-red-500 shadow-[0_0_18px_rgba(239,68,68,0.65)]"
					></span>
					LIVE
					{#if current}
						<span class="ml-2 truncate text-zinc-200">{current.name ?? current.game_id}</span>
					{/if}
				</div>
			</div>

			<div class="pointer-events-none absolute inset-x-0 bottom-0 z-20 p-4">
				<div class="flex items-end justify-between gap-4">
					<div class="min-w-0">
						{#if statusText}
							<div class="text-xs text-zinc-300">{statusText}</div>
						{/if}
						<div class="mt-2 flex flex-wrap items-center gap-2">
							{#if current && current.max_players != null}
								<div
									class="inline-flex items-center rounded-full bg-sky-950/30 px-3 py-1 text-xs font-[var(--dorch-mono)] tracking-wide text-sky-100 ring-1 ring-sky-400/20 backdrop-blur"
								>
									{current.player_count ?? 0} / {current.max_players} players
								</div>
							{/if}

							{#if current && current.monster_total != null}
								<div
									class="inline-flex items-center rounded-full bg-red-950/35 px-3 py-1 text-xs font-[var(--dorch-mono)] tracking-wide text-red-100 ring-1 ring-red-400/20 backdrop-blur"
								>
									{current.monster_kill_count ?? 0} / {current.monster_total} kills
								</div>
							{/if}
						</div>
					</div>
					<div class="shrink-0">
						<div
							class="rounded-lg bg-black/35 px-3 py-2 text-xs text-zinc-200 ring-1 ring-white/10 backdrop-blur"
						>
							Click to open
						</div>
					</div>
				</div>
			</div>

			{#if !items.length}
				<div class="absolute inset-0 z-30 grid place-items-center">
					<div class="rounded-xl bg-black/40 px-4 py-3 text-sm text-zinc-200 ring-1 ring-white/10">
						No live streams right now.
					</div>
				</div>
			{/if}
		</div>
	</a>

	{#if items.length > 1}
		<div class="pointer-events-none absolute inset-0 z-30">
			<div class="pointer-events-auto absolute inset-y-0 left-3 flex items-center">
				<button
					type="button"
					class="grid h-10 w-10 place-items-center rounded-full bg-black/45 text-zinc-100 ring-1 ring-white/15 backdrop-blur hover:bg-black/60 focus-visible:ring-2 focus-visible:ring-zinc-400 focus-visible:outline-none disabled:opacity-50"
					onclick={(e) => {
						e.preventDefault();
						e.stopPropagation();
						void step(-1);
					}}
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
			</div>
			<div class="pointer-events-auto absolute inset-y-0 right-3 flex items-center">
				<button
					type="button"
					class="grid h-10 w-10 place-items-center rounded-full bg-black/45 text-zinc-100 ring-1 ring-white/15 backdrop-blur hover:bg-black/60 focus-visible:ring-2 focus-visible:ring-zinc-400 focus-visible:outline-none disabled:opacity-50"
					onclick={(e) => {
						e.preventDefault();
						e.stopPropagation();
						void step(1);
					}}
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
		</div>
	{/if}
</div>

<style>
	.jv {
		opacity: 0;
		transform: translateZ(0);
		filter: saturate(1.05) contrast(1.06);
		transition:
			opacity 700ms cubic-bezier(0.22, 1, 0.36, 1),
			transform 900ms cubic-bezier(0.22, 1, 0.36, 1),
			filter 900ms cubic-bezier(0.22, 1, 0.36, 1),
			clip-path 900ms cubic-bezier(0.22, 1, 0.36, 1);
		will-change: opacity, transform, filter, clip-path;
	}

	:global(.jv[data-active='true']) {
		opacity: 1;
		z-index: 2;
	}

	:global(.jv[data-active='false']) {
		opacity: 0;
		z-index: 1;
	}

	/* Transition presets (applied via data-kind on BOTH videos). */
	:global(.jv[data-kind='fade'][data-active='true']) {
		transform: scale(1);
		filter: saturate(1.05) contrast(1.06) blur(0px);
	}

	:global(.jv[data-kind='zoom'][data-active='true']) {
		transform: scale(1.02);
		filter: saturate(1.12) contrast(1.07) blur(0px);
	}
	:global(.jv[data-kind='zoom'][data-active='false']) {
		transform: scale(1.08);
		filter: saturate(0.95) contrast(1.02) blur(3px);
	}

	:global(.jv[data-kind='slide-left'][data-active='true']) {
		transform: translateX(0%) scale(1.02);
	}
	:global(.jv[data-kind='slide-left'][data-active='false']) {
		transform: translateX(5%) scale(1.05);
		filter: blur(2px);
	}

	:global(.jv[data-kind='slide-right'][data-active='true']) {
		transform: translateX(0%) scale(1.02);
	}
	:global(.jv[data-kind='slide-right'][data-active='false']) {
		transform: translateX(-5%) scale(1.05);
		filter: blur(2px);
	}

	:global(.jv[data-kind='tilt'][data-active='true']) {
		transform: perspective(900px) rotateX(0deg) rotateY(0deg) scale(1.02);
		filter: saturate(1.12) contrast(1.08);
	}
	:global(.jv[data-kind='tilt'][data-active='false']) {
		transform: perspective(900px) rotateX(2deg) rotateY(-4deg) scale(1.06);
		filter: blur(2px) saturate(0.98);
	}

	:global(.jv[data-kind='wipe'][data-active='true']) {
		clip-path: inset(0% 0% 0% 0% round 0px);
		transform: scale(1.02);
	}
	:global(.jv[data-kind='wipe'][data-active='false']) {
		clip-path: inset(0% 0% 0% 100% round 0px);
		transform: scale(1.04);
		filter: blur(2px);
	}

	:global(.jv[data-kind='glitch'][data-active='true']) {
		transform: translateX(0px) scale(1.02);
		filter: saturate(1.12) contrast(1.1);
	}
	:global(.jv[data-kind='glitch'][data-active='false']) {
		transform: translateX(1px) scale(1.05);
		filter: hue-rotate(12deg) saturate(1.2) contrast(1.12) blur(2px);
	}

	@media (prefers-reduced-motion: reduce) {
		.jv {
			transition: opacity 250ms linear;
		}
		:global(.jv[data-kind][data-active='true']),
		:global(.jv[data-kind][data-active='false']) {
			transform: none;
			filter: none;
			clip-path: none;
		}
	}
</style>
