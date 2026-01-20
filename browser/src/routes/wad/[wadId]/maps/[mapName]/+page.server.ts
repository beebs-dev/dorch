import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

function statusFromUnknown(e: unknown): number | null {
	if (!e || typeof e !== 'object') return null;
	const status = (e as Record<string, unknown>).status;
	return typeof status === 'number' ? status : null;
}

export const load: PageServerLoad = async ({ fetch, params, setHeaders, request }) => {
	const forwardedFor = getTrustedXForwardedFor(request);
	const wadId = params.wadId;
	const mapName = params.mapName;

	const wadinfo = createWadinfoClient(fetch, { forwardedFor });
	try {
		const [map, images] = await Promise.all([
			wadinfo.getWadMap(wadId, mapName),
			wadinfo.listWadMapImages(wadId, mapName).catch(() => [])
		]);

		const merged = {
			...map,
			images: (images?.length ?? 0) > 0 ? images : (map.images ?? [])
		};

		setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=30' });
		return { wadId, mapName, map: merged };
	} catch (e) {
		const status = statusFromUnknown(e);
		if (status !== null) throw error(status, 'Failed to fetch map');
		throw error(500, 'Failed to fetch map');
	}
};
