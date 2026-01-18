<script lang="ts">
	import { onMount } from 'svelte';

	type Props = {
		href: string;
		label?: string;
		ariaLabel?: string;
		className?: string;
	};

	let { href, label = 'PLAY', ariaLabel, className = '' }: Props = $props();

	let hovered = false;
	let buttonEl: HTMLAnchorElement | null = null;

	onMount(() => {
		if (!buttonEl) return;

		const reduceMotionQuery = globalThis.matchMedia?.('(prefers-reduced-motion: reduce)');
		if (reduceMotionQuery?.matches) {
			buttonEl.style.setProperty('--dorch-bg-x', '0px');
			buttonEl.style.setProperty('--dorch-bg-y', '0px');
			return;
		}

		let rafId = 0;
		let lastTime = performance.now();
		let offset = 0;
		let currentSpeed = 256 / 30; // px/s (matches old 30s per tile)
		const baseSpeed = 256 / 30;
		const hoverSpeed = baseSpeed * 3.5;
		const smoothingTauSeconds = 0.18;

		const tick = (now: number) => {
			const dt = Math.min(0.05, Math.max(0, (now - lastTime) / 1000));
			lastTime = now;

			const targetSpeed = hovered ? hoverSpeed : baseSpeed;
			const alpha = 1 - Math.exp(-dt / smoothingTauSeconds);
			currentSpeed = currentSpeed + (targetSpeed - currentSpeed) * alpha;

			offset += currentSpeed * dt;
			// Keep numbers bounded; repeat every tile.
			const tile = 256;
			offset = offset % tile;
			const px = `${offset.toFixed(3)}px`;
			buttonEl?.style.setProperty('--dorch-bg-x', px);
			buttonEl?.style.setProperty('--dorch-bg-y', px);

			rafId = requestAnimationFrame(tick);
		};

		rafId = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(rafId);
	});
</script>

<a
	bind:this={buttonEl}
	{href}
	class={`dorch-play-button flex items-center justify-center rounded-lg px-6 py-2.5 text-lg text-zinc-100 ring-1 ring-red-950/60 ring-inset focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none ${className}`}
	aria-label={ariaLabel ?? label}
	onpointerenter={() => (hovered = true)}
	onpointerleave={() => (hovered = false)}
>
	{label}
</a>

<style>
	.dorch-play-button {
		position: relative;
		isolation: isolate;
		--dorch-tile: 256px;
		--dorch-bg-x: 0px;
		--dorch-bg-y: 0px;
		font-family: 'Orbitron', var(--dorch-mono);
		font-weight: 900;
		outline: 1px solid color-mix(in oklab, var(--color-red-950) 60%, transparent);
		outline-offset: 3px;
		transition:
			outline-color 200ms ease,
			filter 200ms ease;
	}

	.dorch-play-button::before,
	.dorch-play-button::after {
		content: '';
		position: absolute;
		inset: 0;
		border-radius: inherit;
		z-index: -1;
	}

	/* Single panning tiled layer (same animation, speed changes on hover) */
	.dorch-play-button::before {
		background-image: url('/red-single.png');
		background-repeat: repeat;
		background-size: var(--dorch-tile) var(--dorch-tile);
		background-position: var(--dorch-bg-x) var(--dorch-bg-y);
		will-change: background-position;
		z-index: -2;
	}

	/* Fading overlay (dark -> less dark) */
	.dorch-play-button::after {
		background: black;
		opacity: 0.45;
		will-change: opacity;
		transition: opacity 250ms ease;
	}

	.dorch-play-button:hover {
		outline-color: rgba(140, 20, 20, 0.9);
		filter: brightness(1.08);
	}

	.dorch-play-button:hover::after {
		opacity: 0.18;
	}

	@media (prefers-reduced-motion: reduce) {
		.dorch-play-button {
			transition: none;
		}

		.dorch-play-button::before,
		.dorch-play-button::after {
			transition: none;
		}

		/* JS loop short-circuits; keep visuals static */
	}
</style>
