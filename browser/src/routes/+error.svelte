<script lang="ts">
	import { page } from '$app/stores';
	import { base, resolve } from '$app/paths';

	let { errors } = $props();

	const message = $derived(() => {
		const err = Array.isArray(errors) ? errors[0] : errors;
		if (err && typeof err === 'object' && 'message' in err) return String(err.message);
		return 'Unknown error';
	});
</script>

<section class="mx-auto w-full max-w-3xl px-4 py-10">
	<h1 class="text-2xl font-semibold tracking-tight">{$page.status}</h1>
	<p class="mt-2 text-sm text-zinc-300">{message()}</p>
	<div class="mt-6 flex flex-wrap gap-2">
		<a
			href={resolve('/')}
			class="rounded-lg bg-zinc-900 px-3 py-2 text-sm text-zinc-200 ring-1 ring-zinc-800 ring-inset hover:bg-zinc-800"
		>
			Back to browser
		</a>
		<a
			href={base + $page.url.pathname + $page.url.search}
			class="rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-200 ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900"
		>
			Retry
		</a>
	</div>
</section>
