<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import { resolve } from '$app/paths';
	import { humanBytes, wadLabel, withParams } from '$lib/utils/format';

	let { data }: { data: PageData } = $props();

	const sortOptions = [
		{ key: 'featured', label: 'Featured' },
		{ key: 'release_date', label: 'Release Date' },
		{ key: 'most_played', label: 'Most Played' },
		{ key: 'alphabetical', label: 'Alphabetical' }
	] as const;

	function titleFor(wad: PageData['results']['items'][number]): string {
		return wadLabel(wad);
	}

	function mapCountFor(wad: PageData['results']['items'][number]): string {
		const count = wad.content?.counts?.maps;
		if (typeof count === 'number') return String(count);
		const maps = wad.content?.maps;
		if (Array.isArray(maps)) return String(maps.length);
		return '—';
	}
</script>

<svelte:head>
	<title>ONLINE SLAUGHTER - DORCH</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<section class="pt-4 pb-12">
		<div class="mx-auto max-w-3xl text-center">
			<h1 class="text-3xl font-semibold tracking-tight text-balance sm:text-4xl">
				{#if data.q}
					{data.results.full_count.toLocaleString()} WADs matched your query.
				{:else}
					Search among {data.results.full_count.toLocaleString()} WADs.
				{/if}
			</h1>

			<form action="/" method="get" class="mt-6">
				<label class="sr-only" for="home-search">Search WADs</label>
				<div class="flex flex-col gap-3 sm:flex-row sm:items-stretch">
					<input
						id="home-search"
						name="q"
						value={$page.url.searchParams.get('q') ?? ''}
						placeholder="Search by title, author, description, sha1…"
						enterkeyhint="search"
						autocomplete="off"
						spellcheck="false"
						class="w-full rounded-2xl bg-zinc-900/40 px-5 py-4 text-lg text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-500 focus:ring-2 focus:ring-zinc-500 focus:outline-none sm:text-xl"
					/>
					<button
						type="submit"
						class="rounded-2xl bg-zinc-900 px-6 py-4 text-base font-semibold text-zinc-100 ring-1 ring-zinc-800 ring-inset hover:bg-zinc-800"
					>
						Search
					</button>
				</div>
			</form>
		</div>
	</section>

	<div class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex flex-wrap gap-2" role="tablist" aria-label="Sorting">
			{#each sortOptions as opt (opt.key)}
				<a
					class={`rounded-md px-3 py-1.5 text-sm ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900 ${
						data.sort === opt.key ? 'bg-zinc-900 text-zinc-100' : 'text-zinc-300'
					}`}
					href={resolve(withParams($page.url, { sort: opt.key, offset: 0 }))}
					role="tab"
					aria-selected={data.sort === opt.key}
				>
					{opt.label}
				</a>
			{/each}
		</div>
		{#if data.q}
			<div class="text-xs text-zinc-500">
				Showing {data.results.items.length.toLocaleString()} of {data.results.full_count.toLocaleString()}
				results for “<span class="text-zinc-200">{data.q}</span>”
			</div>
		{:else}
			<div class="text-right text-sm text-zinc-400">
				<div>Pick your favorite and play with friends.</div>
			</div>
		{/if}
	</div>

	{#if !data.q}
		<section class="mt-6">
			<div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
				{#each data.featured as item (item.wad.id)}
					<a
						href={resolve(`/wad/${encodeURIComponent(item.wad.id)}`)}
						class="group overflow-hidden rounded-xl ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900"
					>
						<div class="aspect-[16/9] w-full overflow-hidden bg-zinc-900">
							{#if item.images?.[0]?.url}
								<img
									src={item.images[0].url}
									alt=""
									class="h-full w-full object-cover"
									loading="lazy"
								/>
							{:else}
								<div class="h-full w-full bg-gradient-to-br from-zinc-900 to-zinc-800"></div>
							{/if}
						</div>
						<div class="min-w-0 p-4">
							<div class="truncate text-sm font-semibold text-zinc-100 group-hover:underline">
								{titleFor(item.wad)}
							</div>
							<div class="mt-1 text-xs text-zinc-400">
								{item.wad.file?.type ?? '—'} • {humanBytes(item.wad.file?.size ?? null)} •
								{mapCountFor(item.wad)} maps
							</div>
						</div>
					</a>
				{/each}
			</div>
		</section>
	{/if}

	<section class="mt-8">
		{#if data.q}
			<h2 class="text-sm font-semibold text-zinc-200">Search Results</h2>
		{:else}
			<h2 class="text-sm font-semibold text-zinc-200">All WADs</h2>
		{/if}
		<div class="mt-3 overflow-hidden rounded-xl ring-1 ring-zinc-800 ring-inset">
			<ul class="divide-y divide-zinc-800">
				{#each data.results.items as wad (wad.id)}
					<li class="bg-zinc-950/40 hover:bg-zinc-900/40">
						<a href={resolve(`/wad/${encodeURIComponent(wad.id)}`)} class="block px-4 py-3">
							<div class="flex flex-wrap items-center justify-between gap-2">
								<div class="min-w-0">
									<div class="truncate text-sm font-semibold text-zinc-100">
										{titleFor(wad)}
									</div>
									<div class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-400">
										<span>{wad.file?.type ?? '—'}</span>
										<span>{humanBytes(wad.file?.size ?? null)}</span>
										<span>{mapCountFor(wad)} maps</span>
										{#if wad.file?.corrupt}
											<span class="rounded bg-zinc-800 px-2 py-0.5 text-zinc-200">corrupt</span>
										{/if}
									</div>
								</div>
								<div class="flex flex-wrap justify-end gap-2">
									{#each wad.content?.engines_guess ?? [] as e (e)}
										<span
											class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
										>
											{e}
										</span>
									{/each}
									{#each wad.content?.iwads_guess ?? [] as iwad (iwad)}
										<span
											class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-300 ring-1 ring-zinc-800 ring-inset"
										>
											{iwad}
										</span>
									{/each}
								</div>
							</div>
						</a>
					</li>
				{/each}
			</ul>
		</div>

		<div class="mt-4 flex items-center justify-between">
			<a
				class={`rounded-md px-3 py-1.5 text-sm ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900 ${
					data.offset <= 0 ? 'pointer-events-none opacity-50' : ''
				}`}
				href={resolve(withParams($page.url, { offset: Math.max(0, data.offset - data.limit) }))}
				rel="prev"
			>
				Prev
			</a>
			<div class="text-xs text-zinc-500">
				Page {Math.floor(data.offset / data.limit) + 1} of {Math.max(
					1,
					Math.ceil(data.results.full_count / data.limit)
				)}
			</div>
			<a
				class={`rounded-md px-3 py-1.5 text-sm ring-1 ring-zinc-800 ring-inset hover:bg-zinc-900 ${
					data.results.items.length < data.limit ? 'pointer-events-none opacity-50' : ''
				}`}
				href={resolve(withParams($page.url, { offset: data.offset + data.limit }))}
				rel="next"
			>
				Next
			</a>
		</div>
	</section>
</section>
