import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';

export const load: PageServerLoad = async ({ fetch, params, setHeaders }) => {
	const wadId = params.wadId;
	const mapName = params.mapName;

	const wadinfo = createWadinfoClient(fetch);
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
		if (e && typeof e === 'object' && 'status' in e && typeof (e as any).status === 'number') {
			throw error((e as any).status as number, 'Failed to fetch map');
		}
		throw error(500, 'Failed to fetch map');
	}
};
