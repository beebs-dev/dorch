<script lang="ts">
	import type { PageData } from './$types';
	import { invalidateAll } from '$app/navigation';

	let { data }: { data: PageData } = $props();

	let refreshing = $state(false);

	async function refresh() {
		refreshing = true;
		try {
			await invalidateAll();
		} finally {
			refreshing = false;
		}
	}

	function randomIdent(): string {
		const adjectives = [
			'quick',
			'bright',
			'silent',
			'fierce',
			'brave',
			'clever',
			'lucky',
			'wild',
			'calm',
			'proud'
		];
		const nouns = [
			'tiger',
			'eagle',
			'lion',
			'wolf',
			'panther',
			'hawk',
			'fox',
			'bear',
			'dragon',
			'falcon'
		];

		const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
		const noun = nouns[Math.floor(Math.random() * nouns.length)];
		const number = Math.floor(Math.random() * 1000);

		return `${adj}-${noun}-${number}`;
	}

	function difficultyColor(skill: number | undefined): string {
		switch (skill) {
			case 1:
				return 'text-green-400';
			case 2:
				return 'text-yellow-400';
			case 3:
				return 'text-orange-400';
			case 4:
				return 'text-red-400';
			default:
				return 'text-zinc-400';
		}
	}

	function difficultyLabel(skill: number | undefined): string {
		switch (skill) {
			case 1:
				return `I'm Too Young to Die`;
			case 2:
				return `Hurt Me Plenty`;
			case 3:
				return `Ultra-Violence`;
			case 4:
				return `Nightmare!`;
			default:
				return 'Unknown';
		}
	}

	function join(gameId: string) {
		// Stub for now.
		alert(`Join not implemented yet (game_id: ${gameId})`);
	}

	const rows = $derived(() => data.rows ?? []);
	const fetchedAt = $derived(() => (data.fetchedAt ? new Date(data.fetchedAt) : null));
</script>

<svelte:head>
	<title>SERVERS - DORCH</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-6">
	<div class="flex flex-wrap items-end justify-between gap-4">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Servers</h1>
			<div class="mt-1 text-sm text-zinc-400">
				{#if fetchedAt()}
					Last refresh: {fetchedAt()!.toLocaleString()}
				{:else}
					&nbsp;
				{/if}
			</div>
		</div>
		<div class="flex items-center gap-2">
			<button
				type="button"
				class="rounded-md bg-zinc-900 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-zinc-800 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none disabled:opacity-50"
				onclick={refresh}
				disabled={refreshing}
				aria-busy={refreshing}
			>
				{refreshing ? 'Refreshing…' : 'Refresh'}
			</button>
		</div>
	</div>

	{#if data.errorMessage}
		<div
			class="mt-4 rounded-lg bg-zinc-950 p-4 text-sm text-zinc-200 ring-1 ring-red-950/60 ring-inset"
		>
			<div class="font-[var(--dorch-mono)] tracking-wide text-red-200">
				Failed to load server list
			</div>
			<div class="mt-1 text-zinc-300">{data.errorMessage}</div>
		</div>
	{/if}

	<div class="mt-5 overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-red-950/60 ring-inset">
		<div class="overflow-x-auto">
			<table class="min-w-full border-collapse text-left">
				<thead class="bg-red-950/25">
					<tr class="text-xs font-[var(--dorch-mono)] tracking-wide text-zinc-200">
						<th class="px-4 py-3">MAP</th>
						<th class="px-4 py-3">SERVER</th>
						<th class="px-4 py-3">PLAYERS</th>
						<th class="px-4 py-3">KILLS</th>
						<th class="px-4 py-3">Base Game <span class="text-xs text-zinc-400">(IWAD)</span></th>
						<th class="px-4 py-3">ACTIONS</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-red-950/40">
					{#if rows().length === 0}
						<tr>
							<td class="px-4 py-5 text-sm text-zinc-400" colspan="5">No servers found.</td>
						</tr>
					{:else}
						{#each rows() as row (row.game.game_id)}
							<tr class="hover:bg-zinc-900/35">
								<td class="px-4 py-3">
									<div class="flex items-center gap-3">
										{#if row.thumbnailUrl}
											<img
												src={row.thumbnailUrl}
												alt={row.game.info?.current_map ?? 'Map'}
												class="h-10 w-16 shrink-0 rounded-md object-cover ring-1 ring-red-950/60 ring-inset"
												loading="lazy"
											/>
										{:else}
											<div
												class="h-10 w-16 shrink-0 rounded-md bg-zinc-900 ring-1 ring-red-950/60 ring-inset"
											></div>
										{/if}
										<div class="min-w-0">
											<div class="truncate text-sm font-semibold text-zinc-100">
												{row.game.info?.current_map ?? 'UNKNOWN'}
											</div>
											<div class="truncate text-xs text-zinc-400">
												{#if row.game.files?.length}
													{row.pwadName}
												{:else}
													&nbsp;
												{/if}
											</div>
										</div>
									</div>
								</td>

								<td class="px-4 py-3">
									<div class="flex items-center gap-2">
										<div class="min-w-0">
											<div
												class="truncate text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100"
											>
												{row.game.info?.name ?? '(loading...)'}
											</div>
											<div class="mt-0.5 text-xs {difficultyColor(row.game.info?.skill)}">
												{difficultyLabel(row.game.info?.skill)}
											</div>
										</div>
									</div>
								</td>

								<td class="px-4 py-3">
									<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
										{row.game.info?.player_count ?? 0}/{row.game.info?.max_players ?? 0}
									</div>
								</td>
								<td class="px-4 py-3">
									<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
										{#if row.game.info}
											{row.game.info.monster_kill_count}/{row.game.info.monster_count}
										{:else}
											&nbsp;
										{/if}
									</div>
								</td>

								<td class="px-4 py-3">
									<div class="text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100">
											{row.iwadName}
									</div>
								</td>

								<td class="px-4 py-3">
									<div class="flex items-center gap-2">
										<a
											href={`https://dorch.beebs.dev/play/?g=${encodeURIComponent(row.game.game_id)}&identity=${randomIdent()}`}
											type="button"
											class="rounded-md bg-red-950/30 px-3 py-2 text-sm font-[var(--dorch-mono)] tracking-wide text-zinc-100 ring-1 ring-red-950/60 ring-inset hover:bg-red-950/45 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
										>
											Join
										</a>
									</div>
								</td>
							</tr>
						{/each}
					{/if}
				</tbody>
			</table>
		</div>
	</div>
</section>
