import type { PageServerLoad } from './$types';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { GameSummary } from '$lib/types/games';

type ServerRow = {
	game: GameSummary;
	thumbnailUrl?: string;
};

function isPano(img: any): boolean {
	const t = (img?.type ?? img?.kind) as string | null | undefined;
	return t === 'pano';
}

export const load: PageServerLoad = async ({ fetch, setHeaders }) => {
	const dorch = createDorchMasterClient(fetch);
	const wadinfo = createWadinfoClient(fetch);

	let games: GameSummary[] = [];
	let errorMessage: string | null = null;

	try {
		const resp = await dorch.listGames();
		games = resp.games ?? [];
	} catch (e) {
		errorMessage = e instanceof Error ? e.message : 'Failed to fetch servers';
		games = [];
	}

	// Best-effort thumbnail enrichment: try to match a WAD in wadinfo by filename-ish query,
	// then pull the first non-pano screenshot for the current map.
	const MAX_THUMBNAILS = 12;
	let thumbBudget = MAX_THUMBNAILS;

	const rows: ServerRow[] = await Promise.all(
		games.map(async (game): Promise<ServerRow> => {
			if (thumbBudget <= 0) return { game };

			const mapName = game.info?.current_map;
			if (!mapName) return { game };

			const candidate = (game.files?.[0] ?? game.iwad ?? '').trim();
			if (!candidate) return { game };

			const query = candidate.replace(/\.(wad|pk3|zip)$/i, '');

			try {
				// Spend budget only when we actually attempt a wadinfo call.
				thumbBudget -= 1;

				const search = await wadinfo.search({ query, offset: 0, limit: 1 });
				const hit = search.items?.[0];
				if (!hit?.id) return { game };

				const wad = await wadinfo.getWad(hit.id);
				const map = (wad.maps ?? []).find((m) => (m.map ?? '').toLowerCase() === mapName.toLowerCase());
				const images = map?.images ?? [];
				const first = images.find((i) => !isPano(i) && i?.url) ?? images.find((i) => i?.url);
				const url = first?.url as string | undefined;
				return url ? { game, thumbnailUrl: url } : { game };
			} catch {
				return { game };
			}
		})
	);

	// Short TTL; servers list changes frequently.
	setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=5' });

	return {
		rows,
		errorMessage,
		fetchedAt: Date.now()
	};
};
