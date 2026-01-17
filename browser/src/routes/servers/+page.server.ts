import type { PageServerLoad } from './$types';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import type { GameSummary } from '$lib/types/games';

type ServerRow = {
	game: GameSummary;
	thumbnailUrl?: string;
};

export const load: PageServerLoad = async ({ fetch, setHeaders }) => {
	const dorch = createDorchMasterClient(fetch);

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

	// Short TTL; servers list changes frequently.
	setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=5' });

	return {
		rows,
		errorMessage,
		fetchedAt: Date.now()
	};
};
