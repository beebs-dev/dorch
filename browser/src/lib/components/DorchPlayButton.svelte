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
	class={`dorch-play-button flex items-center justify-center rounded-lg bg-red-950/30 px-6 py-2.5 text-lg font-semibold text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-red-950/45 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none ${className}`}
	aria-label={ariaLabel ?? label}
>
	{label}
</a>
