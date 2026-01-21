import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import type { GameSummary } from '$lib/types/games';
import type { WadMeta } from '$lib/types/wadinfo';

type WadWithMaps = {
	id: string;
	meta?: WadMeta | null;
	maps: Array<{ map: string; title?: string | null }>;
};

function stripWadSuffix(title: string): string {
	return title.trim().replace(/\.wad$/i, '').trim();
}

function statusFromUnknown(e: unknown): number | null {
	if (!e || typeof e !== 'object') return null;
	const status = (e as Record<string, unknown>).status;
	return typeof status === 'number' ? status : null;
}

function uniquePreserveOrder(items: Array<string | null | undefined>): string[] {
	const out: string[] = [];
	const seen = new Set<string>();
	for (const v of items) {
		if (!v) continue;
		if (seen.has(v)) continue;
		seen.add(v);
		out.push(v);
	}
	return out;
}

export const load: PageServerLoad = async ({ fetch, params, setHeaders, request }) => {
	const forwardedFor = getTrustedXForwardedFor(request);
	const gameId = params.gameId;
	const dorch = createDorchMasterClient(fetch, { forwardedFor });
	const wadinfo = createWadinfoClient(fetch, { forwardedFor });

	let game: GameSummary | null = null;
	try {
		game = await dorch.getGame(gameId);
	} catch (e) {
		const status = statusFromUnknown(e);
		throw error(status ?? 502, 'Failed to fetch game');
	}

	if (!game) throw error(404, 'Game not found');

	const wadIds = uniquePreserveOrder([game.iwad, ...(game.files ?? [])]);
	const wads: WadWithMaps[] = await Promise.all(
		wadIds.map(async (wadId) => {
			try {
				const wad = await wadinfo.getWad(wadId);
				const singleMapFallbackTitle = wad.maps?.length === 1 && wad.meta?.title ? stripWadSuffix(wad.meta.title) : '';
				return {
					id: wadId,
					meta: wad.meta,
					maps: (wad.maps ?? [])
						.filter((m) => Boolean(m.map))
						.map((m) => ({
							map: m.map,
							title:
								m.title && m.title.trim().length > 0
									? m.title
									: singleMapFallbackTitle || undefined
						}))
				};
			} catch {
				return { id: wadId, meta: null, maps: [] };
			}
		})
	);

	const currentMap = game.info?.current_map ?? null;
	const wadPreference = uniquePreserveOrder([...(game.files ?? []).slice().reverse(), game.iwad]);
	let currentMapWadId: string | null = null;
	if (currentMap) {
		for (const wadId of wadPreference) {
			const w = wads.find((x) => x.id === wadId);
			if (!w) continue;
			if (w.maps.some((m) => m.map === currentMap)) {
				currentMapWadId = wadId;
				break;
			}
		}
		// If we couldn't confirm via wadinfo, fall back to “last PWAD or IWAD”.
		if (!currentMapWadId) {
			currentMapWadId = game.files?.[game.files.length - 1] ?? game.iwad ?? null;
		}
	}

	// Very short TTL; game state changes constantly.
	setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=5' });

	return {
		gameId,
		game,
		wads,
		currentMap,
		currentMapWadId,
		fetchedAt: Date.now()
	};
};
