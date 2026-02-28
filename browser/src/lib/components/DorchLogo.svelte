<script lang="ts">
	import { base, resolve } from '$app/paths';
</script>

<a
	href={resolve('/')}
	aria-label="GIB.GG"
	class="group inline-flex max-w-full items-center gap-2 text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-zinc-400"
>
	<img class="logo-img" src={`${base}/logo.png`} alt="GIB.GG" />
</a>

<style>
	.logo-img {
		height: 32px;
		width: auto;
		max-width: 100%;
		display: block;
		user-select: none;
		filter: drop-shadow(0 0 10px rgba(0, 0, 0, 0.35));
		transition: filter 120ms ease;
	}

	.group:hover .logo-img {
		filter: brightness(1.08) drop-shadow(0 0 14px rgba(0, 0, 0, 0.35));
	}
</style>
