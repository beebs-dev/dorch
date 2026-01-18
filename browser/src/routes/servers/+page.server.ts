import type { PageServerLoad } from './$types';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { GameSummary } from '$lib/types/games';
import type { WadMeta } from '$lib/types/wadinfo';

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

		const currentMap = game.info?.current_map;
		const wadId = game.files?.[game.files.length - 1] ?? game.iwad;
		const thumbnailUrl =
			currentMap && wadId
				? `/servers/${encodeURIComponent(game.game_id)}/thumb?wad_id=${encodeURIComponent(wadId)}&map=${encodeURIComponent(currentMap)}`
				: undefined;

		return {
			game,
			thumbnailUrl,
			iwadName: wadNameById.get(game.iwad) ?? game.iwad,
			pwadName
		};
	});

	// Short TTL; servers list changes frequently.
	setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=5' });

	return {
		rows,
		errorMessage,
		fetchedAt: Date.now()
	};
};
