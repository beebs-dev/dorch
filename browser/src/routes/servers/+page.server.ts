import type { PageServerLoad } from './$types';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { GameSummary } from '$lib/types/games';
import type { MapReference } from '$lib/types/wadinfo';

type ServerRow = {
	game: GameSummary;
	thumbnailUrl?: string;
};

export const load: PageServerLoad = async ({ fetch, setHeaders }) => {
	const dorch = createDorchMasterClient(fetch);
	const wadinfo = createWadinfoClient(fetch);

	let games: GameSummary[];
	let errorMessage: string | null = null;

	try {
		const resp = await dorch.listGames();
		games = resp.games ?? [];
	} catch (e) {
		errorMessage = e instanceof Error ? e.message : 'Failed to fetch servers';
		games = [];
	}

	// Thumbnail enrichment intentionally omitted for now.
	const rows: ServerRow[] = games.map((game) => ({ game }));

	// Resolve all map thumbnails in one trip.
	try {
		const wanted: MapReference[] = [];
		for (const row of rows) {
			const currentMap = row.game.info?.current_map;
			if (!currentMap) continue;

			// Prefer the first PWAD if present, otherwise fall back to IWAD.
			const wad = row.game.files?.[0] ?? row.game.iwad;
			if (!wad?.id) continue;

			wanted.push({ wad_id: wad.id, map: currentMap });
		}

		// De-dupe to keep payload small.
		const seen = new Set<string>();
		const deduped = wanted.filter((r) => {
			const key = `${r.wad_id}:${r.map}`;
			if (seen.has(key)) return false;
			seen.add(key);
			return true;
		});

		if (deduped.length) {
			const resolved = await wadinfo.resolveMapThumbnails(deduped);
			const byKey = new Map(resolved.map((t) => [`${t.wad_id}:${t.map}`, t.url] as const));
			for (const row of rows) {
				const currentMap = row.game.info?.current_map;
				if (!currentMap) continue;
				const wad = row.game.files?.[0] ?? row.game.iwad;
				const url = byKey.get(`${wad.id}:${currentMap}`);
				if (url) row.thumbnailUrl = url;
			}
		}
	} catch {
		// Best-effort only; servers page should still render without thumbnails.
	}

	// Short TTL; servers list changes frequently.
	setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=5' });

	return {
		rows,
		errorMessage,
		fetchedAt: Date.now()
	};
};
