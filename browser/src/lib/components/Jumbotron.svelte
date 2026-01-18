<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { resolve } from '$app/paths';

	type JumbotronItem = {
		game_id: string;
		url: string;
		name?: string;
		player_count?: number;
		max_players?: number;
	};

	export let items: JumbotronItem[] = [];
	export let intervalMs = 5000;

	type TransitionKind = 'fade' | 'zoom' | 'slide-left' | 'slide-right' | 'tilt' | 'wipe' | 'glitch';

	type Slot = 'a' | 'b';

	let mounted = false;
	let current: JumbotronItem | null = null;
	let next: JumbotronItem | null = null;
	let activeSlot: Slot = 'a';
	let currentTransition: TransitionKind = 'fade';
	let switching = false;
	let statusText = '';

	let videoA: HTMLVideoElement | null = null;
	let videoB: HTMLVideoElement | null = null;

	type HlsModule = typeof import('hls.js');
	let Hls: HlsModule['default'] | null = null;
	let hlsA: import('hls.js').default | null = null;
	let hlsB: import('hls.js').default | null = null;

	let switchTimer: number | null = null;

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

	function stopAndResetVideo(video: HTMLVideoElement | null) {
		if (!video) return;
		try {
			video.pause();
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

	async function attachStreamToVideo(
		url: string,
		video: HTMLVideoElement,
		existing: import('hls.js').default | null
	) {
		// Reset previous playback / attachment.
		destroyHls(existing);

		// Some browsers support native HLS playback (Safari). If so, keep it simple.
		if (video.canPlayType('application/vnd.apple.mpegurl')) {
			video.src = url;
			return { hls: null as import('hls.js').default | null };
		}

		await ensureHlsLoaded();
		if (!Hls) return { hls: null };

		if (!Hls.isSupported()) {
			// Last-ditch fallback.
			video.src = url;
			return { hls: null };
		}

		const hls = new Hls({
			enableWorker: true,
			lowLatencyMode: true,
			backBufferLength: 30
		});
		hls.attachMedia(video);
		hls.loadSource(url);
		return { hls };
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

		const { hls } = await attachStreamToVideo(item.url, video, standbyHls());
		setStandbyHls(hls);

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

	async function switchNow() {
		if (switching) return;
		if (!current || !next) return;
		if (!mounted) return;

		switching = true;
		currentTransition = pickTransition();
		setActiveVisualState();

		// If the standby stream isn't ready, try to recover by preloading another.
		const standby = standbyVideo();
		if (standby) {
			await waitForPreload(standby, 1800);
			if (standby.readyState < 2 || standby.error) {
				let recovered = false;
				for (let tries = 0; tries < 3; tries++) {
					const alternative = pickDifferentRandom(items, current?.game_id);
					if (!alternative || alternative.game_id === next.game_id) continue;
					next = alternative;
					recovered = await primeNextStream(next);
					if (recovered) break;
				}
				if (!recovered) {
					// Don't switch to an unbuffered stream.
					switching = false;
					return;
				}
			}
		}

		// Swap active slot.
		activeSlot = activeSlot === 'a' ? 'b' : 'a';
		current = next;
		next = pickDifferentRandom(items, current.game_id);

		// Ensure the newly active video is audible (but keep it fairly subtle for UX).
		const active = activeVideo();
		if (active) {
			active.muted = true;
			await safePlay(active);
		}

		setActiveVisualState();

		// Preload the next candidate immediately (gives us >1–2s headroom).
		await primeNextStream(next);

		// Let the CSS animation run.
		await new Promise((r) => setTimeout(r, 900));
		switching = false;
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

		current = pickDifferentRandom(items, null);
		next = pickDifferentRandom(items, current?.game_id);

		// Load current into active slot.
		const active = activeVideo();
		if (active && current) {
			statusText = 'Loading stream…';
			stopAndResetVideo(active);
			const { hls } = await attachStreamToVideo(current.url, active, activeHls());
			if (activeSlot === 'a') hlsA = hls;
			else hlsB = hls;

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

		switchTimer = window.setInterval(() => {
			void switchNow();
		}, intervalMs);
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

			<div class="pointer-events-none absolute inset-x-0 bottom-0 z-20 p-4">
				<div class="flex items-end justify-between gap-4">
					<div class="min-w-0">
						{#if statusText}
							<div class="text-xs text-zinc-300">{statusText}</div>
						{/if}
						<div class="mt-2 flex flex-wrap items-center gap-2">
							<div
								class="inline-flex min-w-0 items-center gap-2 rounded-full bg-black/45 px-3 py-1 text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-white/10 backdrop-blur"
							>
								<span
									class="h-2 w-2 shrink-0 rounded-full bg-red-500 shadow-[0_0_18px_rgba(239,68,68,0.65)]"
								></span>
								LIVE
								{#if current}
									<span class="ml-2 truncate text-zinc-200">
										{current.name ?? current.game_id}
									</span>
								{/if}
							</div>

							{#if current && current.max_players != null}
								<div
									class="inline-flex items-center rounded-full bg-black/35 px-3 py-1 text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-200 ring-1 ring-white/10 backdrop-blur"
								>
									{current.player_count ?? 0}/{current.max_players} players
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
