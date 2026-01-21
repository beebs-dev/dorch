<script lang="ts">
	import type { PageData } from './$types';
	import { base, resolve } from '$app/paths';
	import { page } from '$app/stores';
	import PanoViewer from '$lib/components/PanoViewer.svelte';
	import DorchPlayButton from '$lib/components/DorchPlayButton.svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import type { WadImage } from '$lib/types/wadinfo';
	import { ellipsize, humanBytes, wadLabel, withParams } from '$lib/utils/format';
	import { showToast } from '$lib/stores/toast';

	let { data }: { data: PageData } = $props();

	type TabKey = 'overview' | 'maps' | 'screenshots' | 'statistics';
	const tabs: Array<{ key: TabKey; label: string }> = [
		{ key: 'overview', label: 'Overview' },
		{ key: 'maps', label: 'Maps' },
		{ key: 'screenshots', label: 'Screenshots' },
		{ key: 'statistics', label: 'Statistics' }
	];

	const wadTitle = $derived(() => wadLabel(data.wad.meta));
	const pageTitle = $derived(() => `${ellipsize(wadTitle(), 64)} - ɢɪʙ.ɢɢ`);

	const wadAuthors = $derived(() => {
		const normalize = (arr: Array<string | null | undefined> | null | undefined) =>
			(arr ?? [])
				.map((a) => (typeof a === 'string' ? a.trim() : ''))
				.filter((a) => a.length > 0);

		const fromMeta = normalize(data.wad.meta.authors);
		if (fromMeta.length) return fromMeta;
		return normalize(data.wad.meta.analysis?.authors);
	});

	function isPano(img: unknown): boolean {
		if (!img || typeof img !== 'object') return false;
		const rec = img as Record<string, unknown>;
		const t = rec.type ?? rec.kind;
		return t === 'pano';
	}

	function firstThumb(map: PageData['wad']['maps'][number]) {
		const images = map.images ?? [];
		return images.find((i) => !isPano(i)) ?? images[0] ?? null;
	}

	const mapsWithAnyImages = $derived(() =>
		data.wad.maps.filter((m) => (m.images?.length ?? 0) > 0)
	);

	const screenshotsCount = $derived(() => {
		let total = 0;
		for (const m of data.wad.maps) total += m.images?.length ?? 0;
		return total;
	});

	const hasMaps = $derived(() => data.wad.maps.length > 0);

	function normalizedSingleMapWadTitle(): string | null {
		if (data.wad.maps.length !== 1) return null;
		const raw = data.wad.meta.title;
		if (!raw) return null;
		const trimmed = raw.trim();
		if (!trimmed) return null;
		const noSuffix = trimmed.replace(/\.wad$/i, '');
		return noSuffix.trim() || null;
	}

	function mapDisplayTitle(m: PageData['wad']['maps'][number]): string {
		if (m.title && m.title.trim().length > 0) return m.title;
		return normalizedSingleMapWadTitle() ?? m.map;
	}

	type ScreenshotPick = { mapName: string; mapTitle?: string | null; image: WadImage };

	const allScreenshotPicks = $derived(() => {
		const picks: ScreenshotPick[] = [];
		for (const m of data.wad.maps) {
			for (const img of m.images ?? []) {
				if (!img?.url) continue;
				if (isPano(img)) continue;
				const title = mapDisplayTitle(m);
				picks.push({ mapName: m.map, mapTitle: title !== m.map ? title : null, image: img });
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

	type FilesTab = { kind: 'file' } | { kind: 'text'; idx: number };
	let selectedFilesTab = $state<FilesTab>({ kind: 'file' });

	const fileNames = $derived(() => {
		const out: string[] = [];
		const seen = new SvelteSet<string>();
		const add = (name: string | null | undefined) => {
			if (!name) return;
			const trimmed = name.trim();
			if (!trimmed) return;
			const key = trimmed.toLowerCase();
			if (seen.has(key)) return;
			seen.add(key);
			out.push(trimmed);
		};

		add(data.wad.meta.filename ?? null);
		for (const n of data.wad.meta.filenames ?? []) add(n);
		return out;
	});

	const fileTabLabel = $derived(() => fileNames()[0] ?? data.wad.meta.file?.type ?? 'File');

	$effect(() => {
		if (selectedFilesTab.kind === 'text' && selectedFilesTab.idx >= textFiles().length) {
			selectedFilesTab = { kind: 'file' };
		}
	});

	function textFileLabel(tf: unknown, idx: number): string {
		const rec = tf && typeof tf === 'object' ? (tf as Record<string, unknown>) : null;
		const name = typeof rec?.name === 'string' ? rec.name : null;
		if (name && name.trim()) return name;
		const source = typeof rec?.source === 'string' ? rec.source : 'text';
		return `${source} #${idx + 1}`;
	}

	let modalImageUrl = $state<string | null>(null);
	let showSha256 = $state(false);
	let isFavorite = $state(false);

	function closeModal() {
		modalImageUrl = null;
	}

	async function copyToClipboard(text: string) {
		try {
			await navigator.clipboard.writeText(text);
		} catch {
			const ta = document.createElement('textarea');
			ta.value = text;
			ta.setAttribute('readonly', '');
			ta.style.position = 'fixed';
			ta.style.left = '-9999px';
			document.body.appendChild(ta);
			ta.select();
			document.execCommand('copy');
			document.body.removeChild(ta);
		}
		showToast('Copied to clipboard');
	}

	function toggleFavorite() {
		// Stub for now.
		isFavorite = !isFavorite;
		console.log('favorite (stub)', { wadId: data.wad.meta.id, favorite: isFavorite });
		showToast(isFavorite ? 'Marked as favorite (stub)' : 'Removed favorite (stub)');
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
	<header class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
		<div class="flex items-center justify-end gap-3">
			<a href={resolve(`/wad/${encodeURIComponent(data.wad.meta.id)}`)} class="min-w-0">
				<h1 class="min-w-0 truncate text-2xl font-semibold tracking-tight">
					{wadTitle()}
				</h1>
			</a>
			<div class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-400">
				<span class="rounded bg-zinc-900 px-2 py-1 ring-1 ring-zinc-800 ring-inset">
					{data.wad.meta.file?.type ?? '—'}
				</span>
				<span class="rounded bg-zinc-900 px-2 py-1 text-zinc-400 ring-1 ring-zinc-800 ring-inset">
					{humanBytes(data.wad.meta.file?.size ?? null)}
				</span>
				<span class="rounded bg-zinc-900 px-2 py-1 text-zinc-400 ring-1 ring-zinc-800 ring-inset">
					{data.wad.maps.length} map(s)
				</span>
			</div>
		</div>
		<div class="flex w-full justify-end sm:w-auto">
			<div class="shrink-0 rounded-xl bg-zinc-950/40 p-1.5 ring-1 ring-red-950/60 ring-inset">
				<div class="flex items-center gap-1">
					<button
						type="button"
						class="flex h-12 w-12 items-center justify-center rounded-lg bg-zinc-900/60 text-zinc-200 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800/70 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
						onclick={toggleFavorite}
						aria-pressed={isFavorite}
						aria-label={isFavorite ? 'Unfavorite WAD' : 'Favorite WAD'}
						title={isFavorite ? 'Unfavorite' : 'Favorite'}
					>
						<svg
							width="20"
							height="20"
							viewBox="0 0 24 24"
							fill={isFavorite ? 'currentColor' : 'none'}
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path
								d="M12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
							></path>
						</svg>
					</button>
					<DorchPlayButton
						href={resolve('/') + `?wad=${encodeURIComponent(data.wad.meta.id)}`}
						label="P L A Y"
						ariaLabel={`Play ${wadTitle()}`}
					/>
				</div>
			</div>
		</div>
	</header>

	<nav
		class="-mx-1 mt-4 flex flex-nowrap gap-1 overflow-x-auto overflow-y-hidden border-b border-zinc-800"
		aria-label="WAD tabs"
	>
		{#each tabs as t (t.key)}
			<a
				href={base + withParams($page.url, { tab: t.key })}
				class={`inline-flex items-center gap-1 px-3 py-2 text-sm font-medium transition-colors focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 focus-visible:outline-none ${
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
				{#if t.key === 'screenshots'}
					<span class="text-xs text-zinc-500">({screenshotsCount()})</span>
				{/if}
			</a>
		{/each}
	</nav>

	{#if data.tab === 'overview'}
		<section
			class={`mt-6 grid grid-cols-1 items-stretch gap-4 ${hasMaps() ? 'lg:grid-cols-2' : ''}`}
		>
			<div class="flex h-full flex-col gap-4">
				<div class="shrink-0 rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
					<h2 class="text-sm font-semibold text-zinc-200">Summary</h2>
					<dl class="mt-3 grid grid-cols-1 gap-2 text-sm">
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">Title</dt>
							<dd class="text-zinc-100">{wadTitle()}</dd>
						</div>
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">Author(s)</dt>
							{#if wadAuthors().length > 0}
								<dd class="flex flex-wrap justify-end gap-2">
									{#each wadAuthors() as author (author)}
										<span
											class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
										>
											{author}
										</span>
									{/each}
								</dd>
							{:else}
								<dd class="text-zinc-400">—</dd>
							{/if}
						</div>
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">WAD ID</dt>
							<dd class="text-xs">
								<button
									type="button"
									class="cursor-pointer font-mono text-xs text-zinc-200"
									onclick={() => copyToClipboard(data.wad.meta.id)}
								>
									{data.wad.meta.id}
								</button>
							</dd>
						</div>
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">SHA1</dt>
							<dd class="text-xs">
								<button
									type="button"
									class="cursor-pointer font-mono text-xs text-zinc-200"
									onclick={() => copyToClipboard(data.wad.meta.sha1)}
								>
									{data.wad.meta.sha1}
								</button>
							</dd>
						</div>
						{#if data.wad.meta.sha256}
							<div class="flex flex-wrap justify-between gap-2">
								<dt class="text-zinc-500">SHA256</dt>
								<dd class="text-xs">
									{#if showSha256}
										<button
											type="button"
											class="cursor-pointer font-mono text-zinc-200"
											onclick={() => copyToClipboard(data.wad.meta.sha256 ?? '')}
										>
											{data.wad.meta.sha256}
										</button>
									{:else}
										<button
											type="button"
											class="text-zinc-400 underline hover:text-zinc-200"
											onclick={() => (showSha256 = true)}
										>
											Show
										</button>
									{/if}
								</dd>
							</div>
						{/if}
					</dl>
				</div>

				<div class="flex min-h-0 flex-1 flex-col rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
					<h2 class="text-sm font-semibold text-zinc-200">Guesses</h2>
					<div class="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2">
						<div>
							<div class="text-xs text-zinc-500">Engines</div>
							<div class="mt-2 flex flex-col items-start gap-2">
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
							<div class="mt-2 flex flex-col items-start gap-2">
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

			{#if hasMaps()}
				<div
					class="group relative h-64 w-full self-start overflow-hidden rounded-lg ring-1 ring-zinc-800 ring-inset sm:h-80 lg:h-[22rem]"
				>
					{#if randomScreenshot?.image?.url}
						<a
							href={resolve(
								`/wad/${encodeURIComponent(data.wad.meta.id)}/maps/${encodeURIComponent(
									randomScreenshot.mapName
								)}`
							)}
							class="block h-full"
							aria-label={`View ${randomScreenshot.mapTitle ? `${randomScreenshot.mapTitle} (${randomScreenshot.mapName})` : randomScreenshot.mapName} details`}
						>
							<img
								src={randomScreenshot.image.url}
								alt=""
								class="block h-full w-full object-cover"
								loading="lazy"
							/>
							<div
								class="pointer-events-none absolute inset-0 flex items-end opacity-0 transition-opacity duration-200 group-hover:opacity-100"
							>
								<div class="w-full bg-zinc-950/70 px-3 py-2 text-sm font-medium text-zinc-100">
									{#if randomScreenshot.mapTitle}
										{randomScreenshot.mapTitle}
										<span class="ml-2 text-xs font-normal text-zinc-300">({randomScreenshot.mapName})</span>
									{:else}
										{randomScreenshot.mapName}
									{/if}
								</div>
							</div>
						</a>
					{:else}
						<div class="h-full w-full bg-gradient-to-br from-zinc-900 to-zinc-800"></div>
					{/if}
				</div>
			{/if}

			<div class={hasMaps() ? 'lg:col-span-2' : ''}>
				<div
					class="flex min-h-0 flex-col rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset"
				>
					<h2 class="text-sm font-semibold text-zinc-200">AI Analysis</h2>
					{#if data.wad.meta.analysis?.description}
						<p class="mt-2 text-sm leading-relaxed text-zinc-300">
							{data.wad.meta.analysis.description}
						</p>
					{:else}
						<div class="mt-2 text-sm text-zinc-400">—</div>
					{/if}

					<div class="mt-4">
						<div class="text-xs text-zinc-500">Tags</div>
						<div class="mt-2 flex flex-wrap gap-2">
							{#each data.wad.meta.analysis?.tags ?? [] as tag (tag)}
								<span
									class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
								>
									{tag}
								</span>
							{/each}
							{#if (data.wad.meta.analysis?.tags?.length ?? 0) === 0}
								<span class="text-sm text-zinc-400">—</span>
							{/if}
						</div>
					</div>
				</div>
			</div>
		</section>

		<section class="mt-4 rounded-xl bg-zinc-950/40 p-4 ring-1 ring-zinc-800 ring-inset">
			<h2 class="text-sm font-semibold text-zinc-200">Files</h2>
			<div class="mt-3 flex flex-wrap gap-2">
				<button
					type="button"
					onclick={() => (selectedFilesTab = { kind: 'file' })}
					class={`inline-flex items-center gap-2 px-3 py-2 text-sm ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900 ${
						selectedFilesTab.kind === 'file' ? 'bg-zinc-900 text-zinc-100' : 'text-zinc-300'
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
						<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
						<polyline points="7 10 12 15 17 10" />
						<line x1="12" y1="15" x2="12" y2="3" />
					</svg>
					<span class="truncate">{fileTabLabel()}</span>
				</button>
				{#each textFiles() as tf, idx (idx)}
					<button
						type="button"
						onclick={() => (selectedFilesTab = { kind: 'text', idx })}
						class={`inline-flex items-center gap-2 px-3 py-2 text-sm ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900 ${
							selectedFilesTab.kind === 'text' && selectedFilesTab.idx === idx
								? 'bg-zinc-900 text-zinc-100'
								: 'text-zinc-300'
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

			{#if selectedFilesTab.kind === 'file'}
				<dl class="mt-4 grid grid-cols-1 gap-2 text-sm">
					<div class="flex flex-wrap justify-between gap-2">
						<dt class="text-zinc-500">Name(s)</dt>
						{#if fileNames().length > 0}
							<dd class="flex flex-wrap justify-end gap-2">
								{#each fileNames() as n (n)}
									<span
										class="rounded bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
									>
										{n}
									</span>
								{/each}
							</dd>
						{:else}
							<dd class="text-zinc-400">—</dd>
						{/if}
					</div>
					<div class="flex flex-wrap justify-between gap-2">
						<dt class="text-zinc-500">Type</dt>
						<dd class="text-zinc-100">{data.wad.meta.file?.type ?? '—'}</dd>
					</div>
					<div class="flex flex-wrap justify-between gap-2">
						<dt class="text-zinc-500">Size</dt>
						<dd class="text-zinc-100">{humanBytes(data.wad.meta.file?.size ?? null)}</dd>
					</div>
					<div class="flex flex-wrap justify-between gap-2">
						<dt class="text-zinc-500">URL</dt>
						{#if data.wad.meta.file?.url}
							<dd class="min-w-0 text-right">
								<a
									href={data.wad.meta.file.url}
									class="truncate text-zinc-300 underline hover:text-zinc-100"
									target="_blank"
									rel="external noreferrer"
								>
									Download
								</a>
							</dd>
						{:else}
							<dd class="text-zinc-400">—</dd>
						{/if}
					</div>
					{#if data.wad.meta.file?.corrupt}
						<div class="flex flex-wrap justify-between gap-2">
							<dt class="text-zinc-500">Status</dt>
							<dd class="text-zinc-100">
								Corrupt{data.wad.meta.file?.corruptMessage
									? `: ${data.wad.meta.file.corruptMessage}`
									: ''}
							</dd>
						</div>
					{/if}
				</dl>
			{:else if textFiles()[selectedFilesTab.idx]}
				<div class="mt-4 overflow-hidden rounded-lg ring-1 ring-zinc-800 ring-inset">
					<div
						class="flex flex-wrap items-center justify-between gap-2 bg-zinc-950 px-3 py-2 text-xs text-zinc-500"
					>
						<div class="min-w-0 truncate">
							{textFileLabel(textFiles()[selectedFilesTab.idx], selectedFilesTab.idx)}
						</div>
						<div class="shrink-0">{textFiles()[selectedFilesTab.idx].source}</div>
					</div>
					<pre
						class="max-h-[420px] overflow-auto bg-zinc-950 p-3 text-xs text-zinc-200">{textFiles()[
							selectedFilesTab.idx
						].contents}</pre>
				</div>
			{/if}
		</section>
	{:else if data.tab === 'maps'}
		<section class="mt-6">
			<div class="overflow-hidden rounded-xl ring-1 ring-zinc-800 ring-inset">
				<ul class="divide-y divide-zinc-800">
					{#each data.wad.maps as m (m.map)}
						<li class="bg-zinc-950/40 hover:bg-zinc-900/40">
							<a
								href={resolve(
									`/wad/${encodeURIComponent(data.wad.meta.id)}/maps/${encodeURIComponent(m.map)}`
								)}
								class="grid grid-cols-1 gap-3 px-4 py-3 sm:grid-cols-[140px_1fr]"
							>
								<div class="overflow-hidden rounded-md ring-1 ring-zinc-800 ring-inset">
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
											{#if mapDisplayTitle(m) !== m.map}
												{mapDisplayTitle(m)}
												<span class="ml-2 text-xs font-normal text-zinc-500">({m.map})</span>
											{:else}
												{m.map}
											{/if}
										</div>
									</div>
									<div class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-400">
										<span>{m.monsters?.total ?? 0} monsters</span>
										<span>{m.items?.total ?? 0} items</span>
										<span>{m.stats?.sectors ?? 0} sectors</span>
										<span>{(m.images?.length ?? 0).toString()} image(s)</span>
										<span>format:{m.format ?? '—'}</span>
										<span>compatibility:{m.compatibility ?? '—'}</span>
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
								<h2 class="text-sm font-semibold text-zinc-200">
									<a
										href={resolve(
											`/wad/${encodeURIComponent(data.wad.meta.id)}/maps/${encodeURIComponent(m.map)}`
										)}
										class="hover:text-zinc-100 hover:underline"
									>
										{#if mapDisplayTitle(m) !== m.map}
											{mapDisplayTitle(m)}
											<span class="ml-2 text-xs font-normal text-zinc-500">({m.map})</span>
										{:else}
											{m.map}
										{/if}
									</a>
								</h2>
								{#if mapDisplayTitle(m) === m.map}
									<div class="text-xs text-zinc-500">{m.map}</div>
								{/if}
							</div>
							<div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
								{#each m.images ?? [] as img (img.id ?? img.url)}
									{#if isPano(img)}
										<div
											class="dorch-pano-glow dorch-pano-label rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset"
										>
											<div class="overflow-hidden rounded-xl">
												<PanoViewer url={img.url} />
											</div>
										</div>
									{:else}
										<div
											class="overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
										>
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
									<td class="py-2 pr-3 pl-3 text-right font-mono text-xs text-zinc-500">{k}</td>
									<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{v}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>

			<div class="overflow-hidden rounded-xl bg-zinc-950/40 ring-1 ring-zinc-800 ring-inset">
				<div class="border-b border-zinc-800 px-4 py-3">
					<h2 class="text-center text-sm font-semibold text-zinc-200">
						Totals <span class="text-zinc-500">(across maps)</span>
					</h2>
				</div>
				<table class="w-full table-fixed text-left text-sm">
					<colgroup>
						<col class="w-1/2" />
						<col class="w-1/2" />
					</colgroup>
					<tbody class="divide-y divide-zinc-800">
						<tr>
							<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Things</td>
							<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{totals().things}</td>
						</tr>
						<tr>
							<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Linedefs</td>
							<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{totals().linedefs}</td>
						</tr>
						<tr>
							<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Sectors</td>
							<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{totals().sectors}</td>
						</tr>
						<tr>
							<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Monsters</td>
							<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{totals().monsters}</td>
						</tr>
						<tr>
							<td class="py-2 pr-3 pl-3 text-right text-zinc-500">Items</td>
							<td class="py-2 pr-3 pl-3 text-left text-zinc-200">{totals().items}</td>
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

<style>
	.dorch-play-button {
		font-family: 'Orbitron', var(--dorch-mono);
		font-weight: 900;
		letter-spacing: 0.06em;
		--dorch-tile: 64px;
		--dorch-tile-scaled: calc(var(--dorch-tile) * 2);
		--dorch-play-glow-color: var(--color-red-900);
		position: relative;
		isolation: isolate;
		overflow: hidden;
		transform: translateZ(0);
		filter: drop-shadow(0 0 6px color-mix(in oklab, var(--dorch-play-glow-color) 45%, transparent));
		animation: dorch-play-idle 12s ease-in-out infinite;
		transition:
			transform 120ms ease,
			filter 180ms ease;
		will-change: transform, filter;
	}

	.dorch-play-button::before {
		content: '';
		position: absolute;
		inset: 0;
		border-radius: inherit;
		background-image: url('/red-single.png');
		background-repeat: repeat;
		background-size: var(--dorch-tile-scaled) var(--dorch-tile-scaled);
		background-position: 0 0;
		opacity: 0.24;
		filter: saturate(1.05) contrast(1.08) brightness(0.96);
		pointer-events: none;
		z-index: 0;
		animation: dorch-play-pan-idle 12s linear infinite;
	}

	.dorch-play-button > :global(*) {
		position: relative;
		z-index: 1;
	}

	.dorch-play-button:hover,
	.dorch-play-button:focus-visible {
		animation: dorch-play-rage 900ms ease-in-out infinite;
		filter: brightness(1.12) saturate(1.16) contrast(1.06)
			drop-shadow(0 0 22px color-mix(in oklab, var(--dorch-play-glow-color) 72%, transparent));
	}

	.dorch-play-button:hover::before,
	.dorch-play-button:focus-visible::before {
		opacity: 0.32;
		filter: saturate(1.35) contrast(1.25) brightness(1.02);
		animation: dorch-play-pan-rage 2500ms linear infinite;
	}

	.dorch-play-button:active {
		animation: dorch-play-hit 160ms ease-out 1;
		transform: scale(0.98);
		filter: brightness(1.12) saturate(1.12)
			drop-shadow(0 0 26px color-mix(in oklab, var(--dorch-play-glow-color) 76%, transparent));
	}

	.dorch-play-button:active::before {
		opacity: 0.32;
		filter: saturate(1.2) contrast(1.2) brightness(1.05);
		animation: dorch-play-pan-hit 180ms linear infinite;
	}

	@keyframes dorch-play-idle {
		0%,
		78% {
			transform: none;
			filter: drop-shadow(
				0 0 6px color-mix(in oklab, var(--dorch-play-glow-color) 45%, transparent)
			);
		}
		82% {
			transform: translateY(-1px) scale(1.01);
			filter: brightness(1.05) saturate(1.06)
				drop-shadow(0 0 12px color-mix(in oklab, var(--dorch-play-glow-color) 56%, transparent));
		}
		86% {
			transform: translateY(0px) scale(1.015);
			filter: brightness(1.07) saturate(1.08)
				drop-shadow(0 0 18px color-mix(in oklab, var(--dorch-play-glow-color) 66%, transparent));
		}
		92% {
			transform: translateY(-1px) scale(1.01);
			filter: brightness(1.05) saturate(1.06)
				drop-shadow(0 0 14px color-mix(in oklab, var(--dorch-play-glow-color) 58%, transparent));
		}
		100% {
			transform: none;
			filter: drop-shadow(
				0 0 6px color-mix(in oklab, var(--dorch-play-glow-color) 45%, transparent)
			);
		}
	}

	@keyframes dorch-play-pan-idle {
		0% {
			opacity: 0.24;
			filter: saturate(1.05) contrast(1.08) brightness(0.96);
			background-position: 0px 0px;
		}
		78% {
			opacity: 0.24;
			filter: saturate(1.05) contrast(1.08) brightness(0.96);
			background-position: calc(var(--dorch-tile-scaled) * 0.65)
				calc(var(--dorch-tile-scaled) * 0.35);
		}
		82% {
			opacity: 0.3;
			filter: saturate(1.2) contrast(1.18) brightness(1.02);
			background-position: calc(var(--dorch-tile-scaled) * 0.78)
				calc(var(--dorch-tile-scaled) * 0.5);
		}
		86% {
			opacity: 0.44;
			filter: saturate(1.35) contrast(1.25) brightness(1.06);
			background-position: calc(var(--dorch-tile-scaled) * 0.9)
				calc(var(--dorch-tile-scaled) * 0.66);
		}
		92% {
			opacity: 0.32;
			filter: saturate(1.22) contrast(1.18) brightness(1.03);
			background-position: calc(var(--dorch-tile-scaled) * 0.96)
				calc(var(--dorch-tile-scaled) * 0.78);
		}
		100% {
			opacity: 0.24;
			filter: saturate(1.05) contrast(1.08) brightness(0.96);
			background-position: var(--dorch-tile-scaled) var(--dorch-tile-scaled);
		}
	}

	@keyframes dorch-play-rage {
		0%,
		100% {
			transform: translate3d(0, 0, 0) scale(1.02);
		}
		25% {
			transform: translate3d(0.5px, -0.5px, 0) scale(1.02);
		}
		50% {
			transform: translate3d(-0.5px, 0.5px, 0) scale(1.02);
		}
		75% {
			transform: translate3d(0.5px, 0.25px, 0) scale(1.02);
		}
	}

	@keyframes dorch-play-glow-rage {
		0%,
		100% {
			opacity: 0.38;
			box-shadow: 0 0 18px 2px color-mix(in srgb, currentColor 24%, transparent);
		}
		25% {
			opacity: 0.5;
			box-shadow: 0 0 24px 3px color-mix(in srgb, currentColor 28%, transparent);
		}
		50% {
			opacity: 0.44;
			box-shadow: 0 0 22px 3px color-mix(in srgb, currentColor 27%, transparent);
		}
		75% {
			opacity: 0.52;
			box-shadow: 0 0 26px 4px color-mix(in srgb, currentColor 29%, transparent);
		}
	}

	@keyframes dorch-play-pan-rage {
		0% {
			background-position: 0px 0px;
		}
		100% {
			background-position: var(--dorch-tile-scaled) var(--dorch-tile-scaled);
		}
	}

	@keyframes dorch-play-pan-hit {
		0% {
			background-position: 0px 0px;
		}
		100% {
			background-position: calc(var(--dorch-tile-scaled) * 2) calc(var(--dorch-tile-scaled) * -1);
		}
	}

	@keyframes dorch-play-hit {
		0% {
			transform: scale(1.02);
			filter: brightness(1.18) saturate(1.2)
				drop-shadow(0 0 28px color-mix(in oklab, var(--dorch-play-glow-color) 82%, transparent));
		}
		100% {
			transform: scale(0.98);
			filter: brightness(1.08) saturate(1.08)
				drop-shadow(0 0 10px color-mix(in oklab, var(--dorch-play-glow-color) 48%, transparent));
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.dorch-play-button,
		.dorch-play-button::before {
			animation: none !important;
			transition: none !important;
		}
	}
</style>
