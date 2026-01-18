<script lang="ts">
	type Props = {
		href: string;
		label?: string;
		ariaLabel?: string;
		className?: string;
	};

	let { href, label = 'PLAY', ariaLabel, className = '' }: Props = $props();
</script>

<a
	href={href}
	class={`dorch-play-button flex items-center justify-center rounded-lg px-6 py-2.5 text-lg font-semibold text-zinc-100 ring-1 ring-red-950/60 ring-inset focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none ${className}`}
	aria-label={ariaLabel ?? label}
>
	{label}
</a>

<style>
	.dorch-play-button {
		outline: 1px solid rgba(239, 68, 68, 0.9);
		outline-offset: 3px;
		background-image: url('/red-single.png');
		background-repeat: repeat;
		background-size: 128px 128px;
		background-position: 0 0;
		animation: dorch-play-button-pan 30s linear infinite;
		will-change: background-position;
	}

	.dorch-play-button:hover {
		filter: brightness(1.08);
	}

	@media (prefers-reduced-motion: reduce) {
		.dorch-play-button {
			animation: none;
		}
	}

	@keyframes dorch-play-button-pan {
		from {
			background-position: 0 0;
		}
		to {
			background-position: 128px 128px;
		}
	}
</style>
