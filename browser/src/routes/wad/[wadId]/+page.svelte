<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import PanoViewer from '$lib/components/PanoViewer.svelte';
	import type { WadImage } from '$lib/types/wadinfo';
	import { humanBytes, shortSha, withParams } from '$lib/utils/format';

	let { data }: { data: PageData } = $props();

	type TabKey = 'overview' | 'maps' | 'screenshots' | 'statistics';
	const tabs: Array<{ key: TabKey; label: string }> = [
		{ key: 'overview', label: 'Overview' },
		{ key: 'maps', label: 'Maps' },
		{ key: 'screenshots', label: 'Screenshots' },
		{ key: 'statistics', label: 'Statistics' }
	];

	const wadTitle = $derived(
		() => data.wad.meta.title ?? `${shortSha(data.wad.meta.sha1)} (untitled)`
	);

	function isPano(img: any): boolean {
		const t = (img?.type ?? img?.kind) as string | null | undefined;
		return t === 'pano';
	}

	function firstThumb(map: PageData['wad']['maps'][number]) {
		const images = map.images ?? [];
		return images.find((i) => !isPano(i)) ?? images[0] ?? null;
	}

	const mapsWithAnyImages = $derived(() =>
		data.wad.maps.filter((m) => (m.images?.length ?? 0) > 0)
	);

	type ScreenshotPick = { mapName: string; image: WadImage };

	const allScreenshotPicks = $derived(() => {
		const picks: ScreenshotPick[] = [];
		for (const m of data.wad.maps) {
			for (const img of m.images ?? []) {
				if (!img?.url) continue;
				if (isPano(img)) continue;
				picks.push({ mapName: m.map, image: img });
			}
		}
		return picks;
	});

	let randomScreenshot = $state<ScreenshotPick | null>(null);
	$effect(() => {
		const picks = allScreenshotPicks();
		randomScreenshot = picks.length ? picks[Math.floor(Math.random() * picks.length)] : null;
	});

	const countEntries = $derived(() => {
		const counts = data.wad.meta.content?.counts ?? {};
		return Object.entries(counts).sort(([a], [b]) => a.localeCompare(b));
	});

	const totals = $derived(() => {
		const init = {
			things: 0,
			linedefs: 0,
			sidedefs: 0,
			vertices: 0,
			sectors: 0,
			segs: 0,
			ssectors: 0,
			nodes: 0,
			monsters: 0,
			items: 0
		};
		for (const m of data.wad.maps) {
			init.things += m.stats?.things ?? 0;
			init.linedefs += m.stats?.linedefs ?? 0;
			init.sidedefs += m.stats?.sidedefs ?? 0;
			init.vertices += m.stats?.vertices ?? 0;
			init.sectors += m.stats?.sectors ?? 0;
			init.segs += m.stats?.segs ?? 0;
			init.ssectors += m.stats?.ssectors ?? 0;
			init.nodes += m.stats?.nodes ?? 0;
			init.monsters += m.monsters?.total ?? 0;
			init.items += m.items?.total ?? 0;
		}
		return init;
	});

	const textFiles = $derived(() => data.wad.meta.text_files ?? []);
	let selectedTextFileIndex = $state(0);

	function textFileLabel(tf: any, idx: number): string {
		const name = tf?.name as string | null | undefined;
		if (name && name.trim()) return name;
		const source = (tf?.source as string | null | undefined) ?? 'text';
		return `${source} #${idx + 1}`;
	}
</script>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<header class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
		<div class="flex items-center justify-end gap-3">
			<h1 class="min-w-0 truncate text-2xl font-semibold tracking-tight">
				{wadTitle()}
			</h1>
			<div class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-400">
				<span class="rounded bg-zinc-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
					{data.wad.meta.file?.type ?? '—'}
				</span>
				<span class="rounded bg-zinc-900 px-2 py-1 text-zinc-200 ring-1 ring-zinc-800 ring-inset">
					{humanBytes(data.wad.meta.file?.size ?? null)}
				</span>
				<span class="rounded bg-zinc-900 px-2 py-1 text-zinc-200 ring-1 ring-zinc-800 ring-inset">
					{data.wad.maps.length} map(s)
				</span>
			</div>
		</div>
		<div class="text-sm text-zinc-400">
			<div class="text-xs">WAD ID</div>
			<div class="font-mono text-xs text-zinc-300">{data.wad.meta.id}</div>
		</div>
	</header>

	<nav
		class="mt-6 -mx-1 flex flex-nowrap gap-1 overflow-x-auto overflow-y-hidden border-b border-zinc-800"
		aria-label="WAD tabs"
	>
		{#each tabs as t (t.key)}
			<a
				href={withParams($page.url, { tab: t.key })}
				class={`inline-flex items-center gap-1 px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 ${
					data.tab === t.key
						? 'border-b-2 border-zinc-100 text-zinc-100'
						: 'border-b-2 border-transparent text-zinc-400 hover:text-zinc-200'
				}`}
				aria-current={data.tab === t.key ? 'page' : undefined}
			>
				<span>{t.label}</span>
				{#if t.key === 'maps'}
					<span class="text-xs text-zinc-500">({data.wad.maps.length})</span>
				{/if}
			</a>
		{/each}
	</nav>

	{#if data.tab === 'overview'}
		<section class="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
			<div class="flex flex-col gap-4">
				<div class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
					<h2 class="text-sm font-semibold text-zinc-200">Summary</h2>
					<dl class="mt-3 grid grid-cols-1 gap-2 text-sm">
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">Title</dt>
							<dd class="text-zinc-100">{data.wad.meta.title ?? '(untitled)'}</dd>
						</div>
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">SHA1</dt>
							<dd class="font-mono text-xs text-zinc-200">{data.wad.meta.sha1}</dd>
						</div>
						{#if data.wad.meta.sha256}
							<div class="flex flex-wrap justify-between gap-2">
								<dt class="text-zinc-500">SHA256</dt>
								<dd class="font-mono text-xs text-zinc-200">{data.wad.meta.sha256}</dd>
							</div>
						{/if}
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">Maps (declared)</dt>
							<dd class="text-zinc-100">{data.wad.meta.content?.maps?.length ?? '—'}</dd>
						</div>
					</dl>
				</div>

				<div class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
					<h2 class="text-sm font-semibold text-zinc-200">Guesses</h2>
					<div class="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2">
						<div>
							<div class="text-xs text-zinc-500">Engines</div>
							<div class="mt-2 flex flex-wrap gap-2">
								{#each data.wad.meta.content?.engines_guess ?? [] as e (e)}
									<span
										class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
									>
										{e}
									</span>
								{/each}
								{#if (data.wad.meta.content?.engines_guess?.length ?? 0) === 0}
									<span class="text-sm text-zinc-400">—</span>
								{/if}
							</div>
						</div>
						<div>
							<div class="text-xs text-zinc-500">IWADs</div>
							<div class="mt-2 flex flex-wrap gap-2">
								{#each data.wad.meta.content?.iwads_guess ?? [] as iwad (iwad)}
									<span
										class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
									>
										{iwad}
									</span>
								{/each}
								{#if (data.wad.meta.content?.iwads_guess?.length ?? 0) === 0}
									<span class="text-sm text-zinc-400">—</span>
								{/if}
							</div>
						</div>
					</div>
				</div>
			</div>

			<div class="group relative overflow-hidden rounded-lg ring-1 ring-zinc-800 ring-inset">
				{#if randomScreenshot?.image?.url}
					<a
						href={`/wad/${encodeURIComponent(data.wad.meta.id)}/maps/${encodeURIComponent(
							randomScreenshot.mapName
						)}`}
						class="block"
						aria-label={`View ${randomScreenshot.mapName} details`}
					>
						<img
							src={randomScreenshot.image.url}
							alt=""
							class="aspect-[16/9] w-full object-cover"
							loading="lazy"
						/>
						<div
							class="pointer-events-none absolute inset-0 flex items-end opacity-0 transition-opacity duration-200 group-hover:opacity-100"
						>
							<div class="w-full bg-zinc-950/70 px-3 py-2 text-sm font-medium text-zinc-100">
								{randomScreenshot.mapName}
							</div>
						</div>
					</a>
				{:else}
					<div class="aspect-[16/9] w-full bg-gradient-to-br from-zinc-900 to-zinc-800"></div>
				{/if}
			</div>
		</section>

		{#if textFiles().length > 0}
			<section class="mt-4 rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
				<h2 class="text-sm font-semibold text-zinc-200">Text files</h2>
				<div class="mt-3 flex flex-wrap gap-2">
					{#each textFiles() as tf, idx (idx)}
						<button
							type="button"
							onclick={() => (selectedTextFileIndex = idx)}
							class={`inline-flex items-center gap-2 px-3 py-2 text-sm ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900 ${
								idx === selectedTextFileIndex ? 'bg-zinc-900 text-zinc-100' : 'text-zinc-300'
							}`}
						>
							<svg
								viewBox="0 0 24 24"
								class="h-4 w-4 text-zinc-400"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
							>
								<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
								<path d="M14 2v6h6" />
								<path d="M8 13h8" />
								<path d="M8 17h8" />
								<path d="M8 9h2" />
							</svg>
							<span class="truncate">{textFileLabel(tf, idx)}</span>
						</button>
					{/each}
				</div>

				{#if textFiles()[selectedTextFileIndex]}
					<div class="mt-4 overflow-hidden rounded-lg ring-1 ring-zinc-800 ring-inset">
						<div
							class="flex flex-wrap items-center justify-between gap-2 bg-zinc-950 px-3 py-2 text-xs text-zinc-500"
						>
							<div class="min-w-0 truncate">
								{textFileLabel(textFiles()[selectedTextFileIndex], selectedTextFileIndex)}
							</div>
							<div class="shrink-0">
								{textFiles()[selectedTextFileIndex].source}
							</div>
						</div>
						<pre
							class="max-h-[420px] overflow-auto bg-zinc-950 p-3 text-xs text-zinc-200">{textFiles()[
								selectedTextFileIndex
							].contents}</pre>
					</div>
				{/if}
			</section>
		{/if}
	{:else if data.tab === 'maps'}
		<section class="mt-6">
			<div class="overflow-hidden rounded-xl ring-1 ring-zinc-800 ring-inset">
				<ul class="divide-y divide-zinc-800">
					{#each data.wad.maps as m (m.map)}
						<li class="bg-zinc-950/40 hover:bg-zinc-900/40">
							<a
								href={`/wad/${encodeURIComponent(data.wad.meta.id)}/maps/${encodeURIComponent(m.map)}`}
								class="grid grid-cols-1 gap-3 px-4 py-3 sm:grid-cols-[140px_1fr]"
							>
								<div class="overflow-hidden rounded-lg bg-zinc-900 ring-1 ring-zinc-800 ring-inset">
									{#if firstThumb(m)?.url}
										<img
											src={firstThumb(m)!.url}
											alt=""
											class="aspect-[16/9] w-full object-cover"
											loading="lazy"
										/>
									{:else}
										<div
											class="aspect-[16/9] w-full bg-gradient-to-br from-zinc-900 to-zinc-800"
										></div>
									{/if}
								</div>
								<div class="min-w-0">
									<div class="flex flex-wrap items-baseline justify-between gap-2">
										<div class="truncate text-sm font-semibold text-zinc-100">
											{m.metadata?.title ?? m.map}
										</div>
										<div class="text-xs text-zinc-500">{m.map}</div>
									</div>
									<div class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-400">
										<span>{m.format ?? '—'}</span>
										<span>{m.compatibility ?? '—'}</span>
										<span>{m.monsters?.total ?? 0} monsters</span>
										<span>{m.items?.total ?? 0} items</span>
										<span>{m.stats?.sectors ?? 0} sectors</span>
										<span>{(m.images?.length ?? 0).toString()} image(s)</span>
									</div>
								</div>
							</a>
						</li>
					{/each}
				</ul>
			</div>
		</section>
	{:else if data.tab === 'screenshots'}
		<section class="mt-6">
			{#if mapsWithAnyImages().length === 0}
				<div
					class="rounded-xl bg-zinc-950/40 p-4 text-sm text-zinc-400 ring-1 ring-zinc-800 ring-inset"
				>
					No screenshots are available for this WAD yet.
				</div>
			{:else}
				<div class="space-y-6">
					{#each mapsWithAnyImages() as m (m.map)}
						<section class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
							<div class="flex flex-wrap items-baseline justify-between gap-2">
								<h2 class="text-sm font-semibold text-zinc-200">{m.map}</h2>
								<div class="text-xs text-zinc-500">{m.metadata?.title ?? ''}</div>
							</div>
							<div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
								{#each m.images ?? [] as img (img.id ?? img.url)}
									{#if isPano(img)}
										<div
											class="overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
										>
											<PanoViewer url={img.url} />
											<div class="px-3 py-2 text-xs text-zinc-500">
												<div class="flex flex-wrap items-center justify-between gap-2">
													<span>pano</span>
													<a
														class="underline hover:text-zinc-300"
														href={img.url}
														target="_blank"
														rel="noreferrer"
													>
														Open image
													</a>
												</div>
											</div>
										</div>
									{:else}
										<div
											class="overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
										>
											<img
												src={img.url}
												alt=""
												class="aspect-[16/9] w-full object-cover"
												loading="lazy"
											/>
											<div class="px-3 py-2 text-xs text-zinc-500">
												{img.type ?? img.kind ?? 'image'}
											</div>
										</div>
									{/if}
								{/each}
							</div>
						</section>
					{/each}
				</div>
			{/if}
		</section>
	{:else if data.tab === 'statistics'}
		<section class="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
			<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
				<div class="border-b border-zinc-800 px-4 py-3">
					<h2 class="text-center text-sm font-semibold text-zinc-200">Counts</h2>
				</div>
				{#if countEntries().length === 0}
					<div class="px-4 py-3 text-sm text-zinc-400">No counts are available.</div>
				{:else}
					<table class="w-full table-fixed text-left text-sm">
						<colgroup>
							<col class="w-1/2" />
							<col class="w-1/2" />
						</colgroup>
						<tbody class="divide-y divide-zinc-800">
							{#each countEntries() as [k, v] (k)}
								<tr>
									<td class="py-2 pl-3 pr-3 text-right font-mono text-xs text-zinc-500">{k}</td>
									<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{v}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>

			<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
				<div class="border-b border-zinc-800 px-4 py-3">
					<h2 class="text-center text-sm font-semibold text-zinc-200">Totals (across maps)</h2>
				</div>
				<table class="w-full table-fixed text-left text-sm">
					<colgroup>
						<col class="w-1/2" />
						<col class="w-1/2" />
					</colgroup>
					<tbody class="divide-y divide-zinc-800">
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Things</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{totals().things}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Linedefs</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{totals().linedefs}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Sectors</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{totals().sectors}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Monsters</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{totals().monsters}</td>
						</tr>
						<tr>
							<td class="py-2 pl-3 pr-3 text-right text-zinc-500">Items</td>
							<td class="py-2 pl-3 pr-3 text-left text-zinc-200">{totals().items}</td>
						</tr>
					</tbody>
				</table>
			</div>
		</section>

		<section class="mt-4">
			<details class="rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
				<summary class="cursor-pointer text-sm font-semibold text-zinc-200 hover:text-zinc-100">
					Raw model
					<span class="ml-2 text-xs font-normal text-zinc-500">(for completeness)</span>
				</summary>
				<pre
					class="mt-3 overflow-auto rounded-lg bg-zinc-950 p-3 text-xs text-zinc-200 ring-1 ring-zinc-800 ring-inset">{JSON.stringify(
						data.wad,
						null,
						2
					)}</pre>
			</details>
		</section>
	{/if}
</section>
