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

	function asSortedBreakdown(v: unknown): Array<[string, number]> {
		if (!v || typeof v !== 'object') return [];
		const entries = Object.entries(v as Record<string, unknown>)
			.map(([k, n]) => [k, typeof n === 'number' ? n : Number(n)] as const)
			.filter(([, n]) => Number.isFinite(n));
		entries.sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
		return entries as Array<[string, number]>;
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

	const monsterBreakdown = $derived(() => asSortedBreakdown(data.map.monsters?.by_type));
	const itemBreakdown = $derived(() => asSortedBreakdown(data.map.items?.by_type));
	const textureList = $derived(() => (data.map.stats?.textures ?? []).filter(Boolean));

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
		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<div class="border-b border-zinc-800 px-4 py-3">
				<h2 class="text-center text-sm font-semibold text-zinc-200">Map Info</h2>
			</div>
			<div class="h-64 overflow-auto">
				<table class="w-full table-fixed text-left text-sm">
					<colgroup>
						<col class="w-1/2" />
						<col class="w-1/2" />
					</colgroup>
					<tbody class="divide-y divide-zinc-800">
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Title</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{data.map.metadata?.title ?? '—'}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Music</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{data.map.metadata?.music ?? '—'}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Source</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{data.map.metadata?.source ?? '—'}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Teleports</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.mechanics?.teleports)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Keys</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.mechanics?.keys)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Secret Exit</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.mechanics?.secret_exit)}</td>
						</tr>
					</tbody>
				</table>
			</div>
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<div class="border-b border-zinc-800 px-4 py-3">
				<h2 class="text-center text-sm font-semibold text-zinc-200">Stats</h2>
			</div>
			<div class="h-64 overflow-auto">
				<table class="w-full table-fixed text-left text-sm">
					<colgroup>
						<col class="w-1/2" />
						<col class="w-1/2" />
					</colgroup>
					<tbody class="divide-y divide-zinc-800">
						{#each statRows() as [label, value] (label)}
							<tr>
								<td class="py-2 pl-3 pr-3 text-right text-zinc-500">{label}</td>
								<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(value)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<div class="border-b border-zinc-800 px-4 py-3">
				<h2 class="text-center text-sm font-semibold text-zinc-200">Difficulty</h2>
			</div>
			<div class="h-64 overflow-auto">
				<table class="w-full table-fixed text-left text-sm">
					<colgroup>
						<col class="w-1/2" />
						<col class="w-1/2" />
					</colgroup>
					<tbody class="divide-y divide-zinc-800">
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">UV monsters</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.difficulty?.uv_monsters)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">HMP monsters</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.difficulty?.hmp_monsters)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">HTR monsters</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.difficulty?.htr_monsters)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">UV items</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.difficulty?.uv_items)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">HMP items</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.difficulty?.hmp_items)}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">HTR items</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{asText(data.map.difficulty?.htr_items)}</td>
						</tr>
					</tbody>
				</table>
			</div>
		</div>
	</section>

	<section class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-3">
		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<div class="border-b border-zinc-800 px-4 py-3">
				<h2 class="text-center text-sm font-semibold text-zinc-200">
					Monsters
					<span class="ml-2 text-xs font-normal text-zinc-500"
						>({data.map.monsters?.total ?? 0} total)</span
					>
				</h2>
			</div>
			{#if monsterBreakdown().length === 0}
				<div class="px-4 py-3 text-sm text-zinc-400">No per-type monster breakdown available.</div>
			{:else}
				<div class="h-64 overflow-auto">
					<table class="w-full table-fixed text-left text-sm">
						<colgroup>
							<col class="w-1/2" />
							<col class="w-1/2" />
						</colgroup>
						<tbody class="divide-y divide-zinc-800">
							{#each monsterBreakdown() as [kind, count] (kind)}
								<tr>
									<td class="py-2 pl-3 pr-3 text-right font-mono text-xs text-zinc-500">{kind}</td>
									<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{count}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<div class="border-b border-zinc-800 px-4 py-3">
				<h2 class="text-center text-sm font-semibold text-zinc-200">
					Items
					<span class="ml-2 text-xs font-normal text-zinc-500"
						>({data.map.items?.total ?? 0} total)</span
					>
				</h2>
			</div>
			{#if itemBreakdown().length === 0}
				<div class="px-4 py-3 text-sm text-zinc-400">No per-type item breakdown available.</div>
			{:else}
				<div class="h-64 overflow-auto">
					<table class="w-full table-fixed text-left text-sm">
						<colgroup>
							<col class="w-1/2" />
							<col class="w-1/2" />
						</colgroup>
						<tbody class="divide-y divide-zinc-800">
							{#each itemBreakdown() as [kind, count] (kind)}
								<tr>
									<td class="py-2 pl-3 pr-3 text-right font-mono text-xs text-zinc-500">{kind}</td>
									<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{count}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<div class="border-b border-zinc-800 px-4 py-3">
				<h2 class="text-center text-sm font-semibold text-zinc-200">
					Textures
					<span class="ml-2 text-xs font-normal text-zinc-500">({textureList().length} unique)</span>
				</h2>
			</div>
			{#if textureList().length === 0}
				<div class="px-4 py-3 text-sm text-zinc-400">No texture list available.</div>
			{:else}
				<div class="h-64 overflow-auto px-4 py-3">
					<ul class="space-y-1 text-sm">
						{#each textureList() as tex (tex)}
							<li class="font-mono text-xs text-zinc-200">{tex}</li>
						{/each}
					</ul>
				</div>
			{/if}
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
