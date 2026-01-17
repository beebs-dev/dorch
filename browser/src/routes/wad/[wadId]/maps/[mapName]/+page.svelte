<script lang="ts">
	import type { PageData } from './$types';
	import PanoViewer from '$lib/components/PanoViewer.svelte';
	import { shortSha } from '$lib/utils/format';

	let { data }: { data: PageData } = $props();

	const wadTitle = $derived(
		() => data.map.wad_meta.title ?? `${shortSha(data.map.wad_meta.sha1)} (untitled)`
	);
	const mapTitle = $derived(() => data.map.metadata?.title ?? data.map.map);

	function isPano(img: any): boolean {
		const t = (img?.type ?? img?.kind) as string | null | undefined;
		return t === 'pano';
	}

	function asText(v: unknown): string {
		if (v === null || v === undefined) return '—';
		if (typeof v === 'boolean') return v ? 'yes' : 'no';
		if (Array.isArray(v)) return v.length ? v.join(', ') : '—';
		return String(v);
	}

	const statRows = $derived(() => {
		const s = data.map.stats ?? {};
		return [
			['Things', s.things],
			['Linedefs', s.linedefs],
			['Sidedefs', s.sidedefs],
			['Vertices', s.vertices],
			['Sectors', s.sectors],
			['Segs', s.segs],
			['SSectors', s.ssectors],
			['Nodes', s.nodes]
		] as const;
	});

	let modalImageUrl = $state<string | null>(null);

	function closeModal() {
		modalImageUrl = null;
	}

	$effect(() => {
		if (!modalImageUrl) return;

		const prevOverflow = document.body.style.overflow;
		document.body.style.overflow = 'hidden';

		const onKeyDown = (e: KeyboardEvent) => {
			if (e.key === 'Escape') closeModal();
		};
		window.addEventListener('keydown', onKeyDown);

		return () => {
			document.body.style.overflow = prevOverflow;
			window.removeEventListener('keydown', onKeyDown);
		};
	});
</script>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<header class="mt-3 flex items-start gap-4">
		<nav class="flex-1 text-sm text-zinc-400" aria-label="Breadcrumb">
			<a
				href={`/wad/${encodeURIComponent(data.wadId)}`}
				class="hover:text-zinc-200 hover:underline"
			>
				{wadTitle()}
			</a>
			<span class="px-2 text-zinc-600">/</span>
			<a
				href={`/wad/${encodeURIComponent(data.wadId)}?tab=maps`}
				class="hover:text-zinc-200 hover:underline"
			>
				Maps
			</a>
			<span class="px-2 text-zinc-600">/</span>
			<span class="font-bold text-zinc-200">{data.mapName}</span>
		</nav>
		<div class="flex flex-wrap justify-end gap-x-3 gap-y-1 text-xs text-zinc-400">
			<span class="rounded bg-zinc-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
				{data.map.format ?? '—'}
			</span>
			<span class="rounded bg-sky-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
				{data.map.compatibility ?? '—'}
			</span>
			<span class="rounded bg-emerald-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
				{data.map.monsters?.total ?? 0} monsters
			</span>
			<span class="rounded bg-violet-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
				{data.map.items?.total ?? 0} items
			</span>
			<span class="rounded bg-amber-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
				{data.map.images?.length ?? 0} image(s)
			</span>
		</div>
	</header>

	<section class="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-3">
		<div class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
			<h2 class="text-sm font-semibold text-zinc-200">Map Info</h2>
			<dl class="mt-3 grid grid-cols-1 gap-2 text-sm">
				<div class="flex flex-wrap justify-between gap-2">
					<dt class="text-zinc-500">Title</dt>
					<dd class="text-zinc-100">{data.map.metadata?.title ?? '—'}</dd>
				</div>
				<div class="flex flex-wrap justify-between gap-2">
					<dt class="text-zinc-500">Music</dt>
					<dd class="text-zinc-100">{data.map.metadata?.music ?? '—'}</dd>
				</div>
				<div class="flex flex-wrap justify-between gap-2">
					<dt class="text-zinc-500">Source</dt>
					<dd class="text-zinc-100">{data.map.metadata?.source ?? '—'}</dd>
				</div>
				<div class="flex flex-wrap justify-between gap-2">
					<dt class="text-zinc-500">Teleports</dt>
					<dd class="text-zinc-100">{asText(data.map.mechanics?.teleports)}</dd>
				</div>
				<div class="flex flex-wrap justify-between gap-2">
					<dt class="text-zinc-500">Keys</dt>
					<dd class="text-zinc-100">{asText(data.map.mechanics?.keys)}</dd>
				</div>
				<div class="flex flex-wrap justify-between gap-2">
					<dt class="text-zinc-500">Secret Exit</dt>
					<dd class="text-zinc-100">{asText(data.map.mechanics?.secret_exit)}</dd>
				</div>
			</dl>
		</div>

		<div class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
			<h2 class="text-sm font-semibold text-zinc-200">Stats</h2>
			<div class="mt-3 divide-y divide-zinc-800">
				{#each statRows() as [label, value] (label)}
					<div class="flex justify-between gap-2 py-2 text-sm">
						<div class="text-zinc-500">{label}</div>
						<div class="text-zinc-100">{asText(value)}</div>
					</div>
				{/each}
			</div>
		</div>

		<div class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
			<h2 class="text-sm font-semibold text-zinc-200">Difficulty</h2>
			<div class="mt-3 divide-y divide-zinc-800">
				<div class="flex justify-between gap-2 py-2 text-sm">
					<div class="text-zinc-500">UV monsters</div>
					<div class="text-zinc-100">{asText(data.map.difficulty?.uv_monsters)}</div>
				</div>
				<div class="flex justify-between gap-2 py-2 text-sm">
					<div class="text-zinc-500">HMP monsters</div>
					<div class="text-zinc-100">{asText(data.map.difficulty?.hmp_monsters)}</div>
				</div>
				<div class="flex justify-between gap-2 py-2 text-sm">
					<div class="text-zinc-500">HTR monsters</div>
					<div class="text-zinc-100">{asText(data.map.difficulty?.htr_monsters)}</div>
				</div>
				<div class="flex justify-between gap-2 py-2 text-sm">
					<div class="text-zinc-500">UV items</div>
					<div class="text-zinc-100">{asText(data.map.difficulty?.uv_items)}</div>
				</div>
				<div class="flex justify-between gap-2 py-2 text-sm">
					<div class="text-zinc-500">HMP items</div>
					<div class="text-zinc-100">{asText(data.map.difficulty?.hmp_items)}</div>
				</div>
				<div class="flex justify-between gap-2 py-2 text-sm">
					<div class="text-zinc-500">HTR items</div>
					<div class="text-zinc-100">{asText(data.map.difficulty?.htr_items)}</div>
				</div>
			</div>
		</div>
	</section>

	<section class="mt-6 rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
		<div class="flex items-center gap-1">
			<h2 class="text-sm font-semibold text-zinc-200">Screenshots</h2>
			<span class="text-sm text-zinc-400">({data.map.images?.length ?? 0})</span>
		</div>
		{#if (data.map.images?.length ?? 0) === 0}
			<div class="mt-3 text-sm text-zinc-400">No screenshots are available for this map yet.</div>
		{:else}
			<div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
				{#each data.map.images ?? [] as img (img.id ?? img.url)}
					{#if isPano(img)}
						<div class="overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset">
							<PanoViewer url={img.url} />
							<div class="px-3 py-2 text-xs text-zinc-500">
								<div class="flex flex-wrap items-center justify-between gap-2">
									<span>panoramic</span>
								</div>
							</div>
						</div>
					{:else}
						<div class="overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset">
							<button
								type="button"
								class="block w-full"
								onclick={() => (modalImageUrl = img.url)}
								aria-label="Open screenshot"
							>
								<img
									src={img.url}
									alt=""
									class="aspect-[16/9] w-full cursor-zoom-in object-cover"
									loading="lazy"
								/>
							</button>
							<div class="px-3 py-2 text-xs text-zinc-500">{img.type ?? img.kind ?? 'image'}</div>
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</section>
</section>

{#if modalImageUrl}
	<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
		<button
			type="button"
			class="absolute inset-0 bg-zinc-950/80"
			onclick={closeModal}
			aria-label="Close screenshot"
		></button>
		<div
			class="relative w-full max-w-5xl overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
			role="dialog"
			aria-modal="true"
			tabindex="-1"
		>
			<div class="max-h-[85vh] overflow-auto">
				<img src={modalImageUrl} alt="" class="w-full object-contain" />
			</div>
		</div>
	</div>
{/if}
