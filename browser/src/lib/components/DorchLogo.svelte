<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<a
	href={resolve('/')}
	class="group inline-flex items-baseline gap-2 text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-zinc-400"
>
	<span class="logo relative text-2xl font-black tracking-[0.25em] select-none">
		<span class="text">GIB.GG</span>
		<span class="alpha">[alpha]</span>
	</span>
</a>

<style>
	.logo {
		font-weight: 900;
		line-height: 1;
		letter-spacing: 0.22em;
	}

	.logo .text {
		font-family: 'Orbitron', var(--dorch-mono);
		display: inline-block;
		padding: 0.05em 0.1em;
		text-transform: uppercase;
		transform: skewX(-8deg);
		color: currentColor;
	}

	.logo .alpha {
		font-family: var(--dorch-sans);
		font-weight: 600;
		font-size: 0.5em;
		opacity: 0.5;
		margin-left: -0.1em;
		margin-top: 0.15em;
		display: inline-block;
		vertical-align: middle;
		pointer-events: none;
	}

	.logo::after {
		content: 'GIB.GG';
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
