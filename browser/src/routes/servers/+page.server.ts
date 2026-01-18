import type { PageServerLoad } from './$types';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { GameSummary } from '$lib/types/games';
import type { MapReference, WadMeta } from '$lib/types/wadinfo';

type ServerRow = {
	game: GameSummary;
	thumbnailUrl?: string;
	iwadName: string;
	pwadName?: string | null;
};

function wadDisplayName(meta: WadMeta): string {
	return meta.title ?? meta.filename ?? meta.filenames?.[0] ?? meta.id;
}

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

	// Enrich wad names (best-effort). dorch-master now returns only UUIDs for iwad/files.
	const wantedWadIds = new Set<string>();
	for (const game of games) {
		if (game.iwad) wantedWadIds.add(game.iwad);
		for (const file of game.files ?? []) {
			if (file) wantedWadIds.add(file);
		}
	}

	const wadNameById = new Map<string, string>();
	if (wantedWadIds.size) {
		await Promise.all(
			[...wantedWadIds].map(async (wadId) => {
				try {
					const wad = await wadinfo.getWad(wadId);
					wadNameById.set(wadId, wadDisplayName(wad.meta));
				} catch {
					// Best-effort only; fall back to showing the UUID.
				}
			})
		);
	}

	const rows: ServerRow[] = games.map((game) => {
		const pwadNames = (game.files ?? [])
			.filter(Boolean)
			.map((fileId) => wadNameById.get(fileId) ?? fileId)
			.filter((s) => s != "Doom Shareware v1.9"); // Omit common PWAD name
		const pwadName = pwadNames.length ? pwadNames.join(' | ') : null;
		return {
			game,
			iwadName: wadNameById.get(game.iwad) ?? game.iwad,
			pwadName
		};
	});

	// Resolve all map thumbnails in one trip.
	try {
		const wanted: MapReference[] = [];
		for (const row of rows) {
			const currentMap = row.game.info?.current_map;
			if (!currentMap) continue;

			// Prefer the first PWAD if present, otherwise fall back to IWAD.
			const wadId = row.game.files?.[row.game.files.length - 1] ?? row.game.iwad;
			if (!wadId) continue;

			wanted.push({ wad_id: wadId, map: currentMap });
		}

		// De-dupe to keep payload small.
		const seen = new Set<string>();
		const deduped = wanted.filter((r) => {
			const key = `${r.wad_id}:${r.map}`;
			if (seen.has(key)) return false;
			seen.add(key);
			return true;
		});
		console.log('De-duped map thumbnails for servers page:', {deduped});

		if (deduped.length) {
			const resolved = await wadinfo.resolveMapThumbnails(deduped);
			console.log('Resolved map thumbnails for servers page:', {resolved, wanted});
			const byKey = new Map(resolved.map((t) => [`${t.wad_id}:${t.map}`, t.url] as const));
			for (const row of rows) {
				const currentMap = row.game.info?.current_map;
				if (!currentMap) continue;
				const wadId = row.game.files?.[row.game.files.length - 1] ?? row.game.iwad;
				if (!wadId) continue;
				const url = byKey.get(`${wadId}:${currentMap}`);
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
