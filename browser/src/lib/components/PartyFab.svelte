<script lang="ts">
	import { showToast } from '$lib/stores/toast';

	function onClick() {
		showToast('Party & messaging coming soon.');
	}
</script>

<div class="fixed bottom-6 right-6 z-[900]">
	<button
		type="button"
		onclick={onClick}
		aria-label="Party and messaging"
		class="group cursor-pointer inline-flex h-14 w-14 items-center justify-center rounded-full bg-gradient-to-br from-red-700 to-red-600 text-white shadow-lg shadow-red-950/40 ring-1 ring-red-500/30 transition hover:from-red-600 hover:to-red-500 hover:shadow-red-950/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-200"
	>
		<svg
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			class="h-6 w-6"
			aria-hidden="true"
		>
			<path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4z" />
			<path d="M8 10h.01" />
			<path d="M12 10h.01" />
			<path d="M16 10h.01" />
		</svg>
	</button>

	<div
		class="pointer-events-none absolute right-full top-1/2 hidden -translate-y-1/2 pr-3 group-hover:block"
	>
		<div class="rounded-md bg-zinc-950 px-2 py-1 text-xs text-zinc-200 ring-1 ring-zinc-800">
			Party
		</div>
	</div>
</div>
