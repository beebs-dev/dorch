<script lang="ts">
	import type { PageData } from './$types';
	import { resolve } from '$app/paths';
	import PanoViewer from '$lib/components/PanoViewer.svelte';
	import { ellipsize, wadLabel } from '$lib/utils/format';

	let { data }: { data: PageData } = $props();

	const wadTitle = $derived(() => wadLabel(data.map.wad_meta));
	const mapDisplayTitle = $derived(() => (data.map.title?.length ?? 0) > 0 ? data.map.title : data.mapName);
	const pageTitle = $derived(
		() => `${ellipsize(wadTitle(), 64)} // ${ellipsize(mapDisplayTitle(), 48)} - ɢɪʙ.ɢɢ`
	);

	const mapAuthors = $derived(() => {
		const normalize = (arr: Array<string | null | undefined> | null | undefined) =>
			(arr ?? [])
				.map((a) => (typeof a === 'string' ? a.trim() : ''))
				.filter((a) => a.length > 0);

		const fromMeta = normalize(data.map.wad_meta?.authors);
		if (fromMeta.length) return fromMeta;
		return normalize(data.map.analysis?.authors);
	});

	function isPano(img: unknown): boolean {
		if (!img || typeof img !== 'object') return false;
		const rec = img as Record<string, unknown>;
		const t = rec.type ?? rec.kind;
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
	const textureBreakdown = $derived(() => {
		const raw = (data.map.stats as Record<string, unknown> | undefined)?.textures;
		// Back-compat: older payloads used `string[]`.
		if (Array.isArray(raw)) {
			const counts: Record<string, number> = {};
			for (const v of raw) {
				if (!v) continue;
				const key = String(v);
				counts[key] = (counts[key] ?? 0) + 1;
			}
			return asSortedBreakdown(counts);
		}
		return asSortedBreakdown(raw);
	});

	type TopKey = 'mapInfo' | 'stats' | 'difficulty';
	type BottomKey = 'monsters' | 'items' | 'textures';

	let topExpanded = $state<Record<TopKey, boolean>>({
		mapInfo: true,
		stats: true,
		difficulty: true
	});

	let bottomExpanded = $state<Record<BottomKey, boolean>>({
		monsters: true,
		items: true,
		textures: true
	});

	// Reset defaults when navigating between maps.
	$effect(() => {
		const deps = `${data.wadId}:${data.mapName}`;
		if (!deps) return;
		topExpanded = { mapInfo: true, stats: true, difficulty: true };
		bottomExpanded = { monsters: true, items: true, textures: true };
	});

	function isTopExpanded(key: TopKey): boolean {
		return topExpanded[key] ?? false;
	}

	function isBottomExpanded(key: BottomKey): boolean {
		return bottomExpanded[key] ?? false;
	}

	function toggleTop(key: TopKey) {
		topExpanded[key] = !topExpanded[key];
	}

	function toggleBottom(key: BottomKey) {
		bottomExpanded[key] = !bottomExpanded[key];
	}

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

<svelte:head>
	<title>{pageTitle()}</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<header class="mt-3 flex flex-wrap items-start justify-between gap-4">
		<nav aria-label="Breadcrumb" class="min-w-0 flex-1">
			<ol class="flex min-w-0 flex-wrap items-baseline gap-2">
				<li class="min-w-0">
					<a
						href={resolve(`/wad/${encodeURIComponent(data.wadId)}`)}
						class="block min-w-0 truncate text-xs font-[var(--dorch-mono)] font-medium tracking-wide text-zinc-400 hover:text-zinc-200 focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-500"
					>
						{wadTitle()}
					</a>
				</li>
				<li aria-hidden="true" class="shrink-0 text-xs text-zinc-600">
					/
				</li>
				<li class="shrink-0">
					<a
						href={resolve(`/wad/${encodeURIComponent(data.wadId)}?tab=maps`)}
						class="text-xs font-[var(--dorch-mono)] font-medium tracking-wide text-zinc-400 hover:text-zinc-200 focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-500"
					>
						MAPS
					</a>
				</li>
				<li aria-hidden="true" class="shrink-0 text-xs text-zinc-600">
					/
				</li>
				<li class="min-w-0" aria-current="page">
					<h1 class="truncate text-2xl font-semibold tracking-tight text-zinc-100">
						{#if data.map.title}
							{data.map.title}
							<span class="ml-2 text-base font-normal text-zinc-500">({data.mapName})</span>
						{:else}
							{data.mapName}
						{/if}
					</h1>
				</li>
			</ol>
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

	<section class="mt-6 rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
		<h2 class="text-sm font-semibold text-zinc-200">AI Analysis</h2>
		{#if data.map.analysis?.description}
			<p class="mt-2 text-sm leading-relaxed text-zinc-300">
				{data.map.analysis.description}
			</p>
		{:else}
			<div class="mt-2 text-sm text-zinc-400">—</div>
		{/if}

		<div class="mt-4">
			<div class="text-xs text-zinc-500">Tags</div>
			<div class="mt-2 flex flex-wrap gap-2">
				{#each data.map.analysis?.tags ?? [] as tag (tag)}
					<span
						class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
					>
						{tag}
					</span>
				{/each}
				{#if (data.map.analysis?.tags?.length ?? 0) === 0}
					<span class="text-sm text-zinc-400">—</span>
				{/if}
			</div>
		</div>
	</section>

	<section class="mt-4 overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
		<div class="border-b border-zinc-800 px-4 py-3">
			<h2 class="text-center text-sm font-semibold text-zinc-200">
				Screenshots
				<span class="ml-2 text-xs font-normal text-zinc-500">({data.map.images?.length ?? 0})</span>
			</h2>
		</div>
		<div class="p-4">
			{#if (data.map.images?.length ?? 0) === 0}
				<div class="text-sm text-zinc-400">No screenshots are available for this map yet.</div>
			{:else}
				<div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
					{#each data.map.images ?? [] as img (img.id ?? img.url)}
						{#if isPano(img)}
							<div
								class="dorch-pano-glow dorch-pano-label rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset"
							>
								<div class="overflow-hidden rounded-xl">
									<PanoViewer url={img.url} />
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
							</div>
						{/if}
					{/each}
				</div>
			{/if}
		</div>
	</section>

	<section class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-3">
		<div
			class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset"
		>
			<button
				type="button"
				class="relative flex w-full cursor-pointer items-center justify-center border-b border-zinc-800 px-4 py-3 transition-colors hover:bg-zinc-900/40 focus-visible:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset"
				onclick={() => toggleTop('mapInfo')}
				aria-expanded={isTopExpanded('mapInfo')}
			>
				<h2 class="text-center text-sm font-semibold text-zinc-200">Map Info</h2>
				{#if !isTopExpanded('mapInfo')}
					<span class="absolute right-4 text-xs font-normal text-zinc-500">expand</span>
				{/if}
			</button>
			{#if isTopExpanded('mapInfo')}
				<div class="h-64 overflow-auto">
					<table class="w-full table-fixed text-left text-sm">
						<colgroup>
							<col class="w-1/2" />
							<col class="w-1/2" />
						</colgroup>
						<tbody class="divide-y divide-zinc-800">
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Title</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{data.map.metadata?.title ?? '—'}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Music</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{data.map.metadata?.music ?? '—'}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Source</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{data.map.metadata?.source ?? '—'}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Author(s)</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{asText(mapAuthors())}</td>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Teleports</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.mechanics?.teleports)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Keys</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.mechanics?.keys)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Secret Exit</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.mechanics?.secret_exit)}</td
								>
							</tr>
						</tbody>
					</table>
				</div>
			{/if}
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<button
				type="button"
				class="relative flex w-full cursor-pointer items-center justify-center border-b border-zinc-800 px-4 py-3 transition-colors hover:bg-zinc-900/40 focus-visible:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset"
				onclick={() => toggleTop('stats')}
				aria-expanded={isTopExpanded('stats')}
			>
				<h2 class="text-center text-sm font-semibold text-zinc-200">Stats</h2>
				{#if !isTopExpanded('stats')}
					<span class="absolute right-4 text-xs font-normal text-zinc-500">expand</span>
				{/if}
			</button>
			{#if isTopExpanded('stats')}
				<div class="h-64 overflow-auto">
					<table class="w-full table-fixed text-left text-sm">
						<colgroup>
							<col class="w-1/2" />
							<col class="w-1/2" />
						</colgroup>
						<tbody class="divide-y divide-zinc-800">
							{#each statRows() as [label, value] (label)}
								<tr>
									<td class="py-2 pr-3 pl-3 text-right text-zinc-500">{label}</td>
									<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{asText(value)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<button
				type="button"
				class="relative flex w-full cursor-pointer items-center justify-center border-b border-zinc-800 px-4 py-3 transition-colors hover:bg-zinc-900/40 focus-visible:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset"
				onclick={() => toggleTop('difficulty')}
				aria-expanded={isTopExpanded('difficulty')}
			>
				<h2 class="text-center text-sm font-semibold text-zinc-200">Difficulty</h2>
				{#if !isTopExpanded('difficulty')}
					<span class="absolute right-4 text-xs font-normal text-zinc-500">expand</span>
				{/if}
			</button>
			{#if isTopExpanded('difficulty')}
				<div class="h-64 overflow-auto">
					<table class="w-full table-fixed text-left text-sm">
						<colgroup>
							<col class="w-1/2" />
							<col class="w-1/2" />
						</colgroup>
						<tbody class="divide-y divide-zinc-800">
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">UV monsters</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.difficulty?.uv_monsters)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">HMP monsters</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.difficulty?.hmp_monsters)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">HTR monsters</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.difficulty?.htr_monsters)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">UV items</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.difficulty?.uv_items)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">HMP items</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.difficulty?.hmp_items)}</td
								>
							</tr>
							<tr>
								<td class="py-2 pr-3 pl-3 text-right text-zinc-500">HTR items</td>
								<td class="py-2 pr-3 pl-3 text-left text-zinc-200"
									>{asText(data.map.difficulty?.htr_items)}</td
								>
							</tr>
						</tbody>
					</table>
				</div>
			{/if}
		</div>
	</section>

	<section class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-3">
		<div
			class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset"
		>
			<button
				type="button"
				class="relative flex w-full cursor-pointer items-center justify-center border-b border-zinc-800 px-4 py-3 transition-colors hover:bg-zinc-900/40 focus-visible:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset"
				onclick={() => toggleBottom('monsters')}
				aria-expanded={isBottomExpanded('monsters')}
			>
				<h2 class="text-center text-sm font-semibold text-zinc-200">
					Monsters
					<span class="ml-2 text-xs font-normal text-zinc-500"
						>({data.map.monsters?.total ?? 0} total)</span
					>
				</h2>
				{#if !isBottomExpanded('monsters')}
					<span class="absolute right-4 text-xs font-normal text-zinc-500">expand</span>
				{/if}
			</button>
			{#if isBottomExpanded('monsters')}
				{#if monsterBreakdown().length === 0}
					<div class="px-4 py-3 text-sm text-zinc-400">
						No per-type monster breakdown available.
					</div>
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
										<td class="py-2 pr-3 pl-3 text-right font-mono text-xs text-zinc-500">{kind}</td
										>
										<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{count}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			{/if}
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<button
				type="button"
				class="relative flex w-full cursor-pointer items-center justify-center border-b border-zinc-800 px-4 py-3 transition-colors hover:bg-zinc-900/40 focus-visible:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset"
				onclick={() => toggleBottom('items')}
				aria-expanded={isBottomExpanded('items')}
			>
				<h2 class="text-center text-sm font-semibold text-zinc-200">
					Items
					<span class="ml-2 text-xs font-normal text-zinc-500"
						>({data.map.items?.total ?? 0} total)</span
					>
				</h2>
				{#if !isBottomExpanded('items')}
					<span class="absolute right-4 text-xs font-normal text-zinc-500">expand</span>
				{/if}
			</button>
			{#if isBottomExpanded('items')}
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
										<td class="py-2 pr-3 pl-3 text-right font-mono text-xs text-zinc-500">{kind}</td
										>
										<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{count}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			{/if}
		</div>

		<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
			<button
				type="button"
				class="relative flex w-full cursor-pointer items-center justify-center border-b border-zinc-800 px-4 py-3 transition-colors hover:bg-zinc-900/40 focus-visible:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none focus-visible:ring-inset"
				onclick={() => toggleBottom('textures')}
				aria-expanded={isBottomExpanded('textures')}
			>
				<h2 class="text-center text-sm font-semibold text-zinc-200">
					Textures
					<span class="ml-2 text-xs font-normal text-zinc-500"
						>(use counts)</span
					>
				</h2>
				{#if !isBottomExpanded('textures')}
					<span class="absolute right-4 text-xs font-normal text-zinc-500">expand</span>
				{/if}
			</button>
			{#if isBottomExpanded('textures')}
				{#if textureBreakdown().length === 0}
					<div class="px-4 py-3 text-sm text-zinc-400">No texture list available.</div>
				{:else}
					<div class="h-64 overflow-auto">
						<table class="w-full table-fixed text-left text-sm">
							<colgroup>
								<col class="w-1/2" />
								<col class="w-1/2" />
							</colgroup>
							<tbody class="divide-y divide-zinc-800">
								{#each textureBreakdown() as [tex, count] (tex)}
									<tr>
										<td class="py-2 pr-3 pl-3 text-right font-mono text-xs text-zinc-500">{tex}</td>
										<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{count}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			{/if}
		</div>
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
