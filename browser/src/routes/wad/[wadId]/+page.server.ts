import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';

const allowedTabs = new Set(['overview', 'maps', 'screenshots', 'statistics']);

export const load: PageServerLoad = async ({ fetch, params, url, setHeaders }) => {
	const wadId = params.wadId;
	const tabParam = (url.searchParams.get('tab') ?? 'overview').toLowerCase();
	const tab = allowedTabs.has(tabParam) ? tabParam : 'overview';

	const wadinfo = createWadinfoClient(fetch);
	try {
		const wad = await wadinfo.getWad(wadId);
		setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=30' });
		return { wadId, tab, wad };
	} catch (e) {
		if (e && typeof e === 'object' && 'status' in e && typeof (e as any).status === 'number') {
			const status = (e as any).status as number;
			throw error(status, 'Failed to fetch WAD');
		}
		throw error(500, 'Failed to fetch WAD');
	}
};
