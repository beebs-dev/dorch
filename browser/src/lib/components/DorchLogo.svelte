<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<a
	href={resolve('/')}
	class="group inline-flex items-baseline gap-2 text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-zinc-400"
>
	<span class="logo relative text-2xl font-black tracking-[0.25em] select-none">
		<span class="text">DORCH</span>
	</span>
</a>

<style>
	.logo {
		line-height: 1;
		letter-spacing: 0.22em;
	}

	.logo .text {
		display: inline-block;
		padding: 0.05em 0.1em;
		text-transform: uppercase;
		transform: skewX(-8deg);
		color: currentColor;
	}

	.logo::after {
		content: 'DORCH';
		position: absolute;
		inset: 0;
		transform: skewX(-8deg);
		color: currentColor;
		filter: blur(8px);
		opacity: 0.18;
		pointer-events: none;
	}

	.group:hover .text {
		filter: brightness(1.15);
	}
</style>
