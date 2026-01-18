import { env } from '$env/dynamic/private';
import { createDorchMasterClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { RequestHandler } from './$types';

function getMasterBaseUrl(): string {
	const base = env.MASTER_BASE_URL;
	if (!base) throw new Error('Missing required private env var MASTER_BASE_URL');
	return base.endsWith('/') ? base : `${base}/`;
}

function buildMasterUrl(path: string): URL {
	return new URL(path.replace(/^\//, ''), getMasterBaseUrl());
}

function copyProxyHeaders(src: Headers): Headers {
	const headers = new Headers(src);

	// Strip hop-by-hop headers.
	for (const name of [
		'connection',
		'keep-alive',
		'proxy-authenticate',
		'proxy-authorization',
		'te',
		'trailer',
		'transfer-encoding',
		'upgrade',
		'set-cookie'
	]) {
		headers.delete(name);
	}

	return headers;
}

async function resolveWadinfoThumbnailUrl(
	fetchFn: typeof fetch,
	wadId: string,
	mapName: string
): Promise<string | null> {
	const wadinfo = createWadinfoClient(fetchFn);
	const items = await wadinfo.resolveMapThumbnails([{ wad_id: wadId, map: mapName }]);
	return items[0]?.url ?? null;
}

export const GET: RequestHandler = async ({ fetch, params, url }) => {
	const gameId = params.gameId;

	// 1) Prefer dorch-master liveshot.
	const liveshotUrl = buildMasterUrl(`/game/${encodeURIComponent(gameId)}/liveshot`);
	const liveshotRes = await fetch(liveshotUrl);
	if (liveshotRes.status === 200) {
		return new Response(liveshotRes.body, {
			status: 200,
			headers: copyProxyHeaders(liveshotRes.headers)
		});
	}

	// If dorch-master returns something other than 404, propagate it.
	if (liveshotRes.status !== 404) {
		return new Response(await liveshotRes.text(), {
			status: liveshotRes.status,
			headers: copyProxyHeaders(liveshotRes.headers)
		});
	}

	// 2) Fast fallback: allow callers to supply wad_id/map to avoid a listGames lookup.
	const wadId = url.searchParams.get('wad_id');
	const mapName = url.searchParams.get('map');
	if (wadId && mapName) {
		try {
			const resolvedUrl = await resolveWadinfoThumbnailUrl(fetch, wadId, mapName);
			if (resolvedUrl) {
				return new Response(null, {
					status: 307,
					headers: { location: resolvedUrl }
				});
			}
		} catch {
			// Fall through to slower fallback.
		}
	}

	// 3) Slower fallback: lookup game -> map + wad ids, then resolve via wadinfo.
	try {
		const dorch = createDorchMasterClient(fetch);
		const resp = await dorch.listGames();
		const game = (resp.games ?? []).find((g) => g.game_id === gameId);
		const currentMap = game?.info?.current_map;
		const fallbackWadId = game?.files?.[game.files.length - 1] ?? game?.iwad;

		if (currentMap && fallbackWadId) {
			const resolvedUrl = await resolveWadinfoThumbnailUrl(fetch, fallbackWadId, currentMap);
			if (resolvedUrl) {
				return new Response(null, {
					status: 307,
					headers: { location: resolvedUrl }
				});
			}
		}
	} catch {
		// ignore
	}

	return new Response('Thumbnail not found', { status: 404 });
};
