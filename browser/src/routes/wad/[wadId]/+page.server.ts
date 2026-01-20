import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

const allowedTabs = new Set(['overview', 'maps', 'screenshots', 'statistics']);

function statusFromUnknown(e: unknown): number | null {
	if (!e || typeof e !== 'object') return null;
	const status = (e as Record<string, unknown>).status;
	return typeof status === 'number' ? status : null;
}

export const load: PageServerLoad = async ({ fetch, params, url, setHeaders, request }) => {
	const forwardedFor = getTrustedXForwardedFor(request);
	const wadId = params.wadId;
	const tabParam = (url.searchParams.get('tab') ?? 'overview').toLowerCase();
	const tab = allowedTabs.has(tabParam) ? tabParam : 'overview';

	const wadinfo = createWadinfoClient(fetch, { forwardedFor });
	try {
		const wad = await wadinfo.getWad(wadId);
		setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=30' });
		return { wadId, tab, wad };
	} catch (e) {
		const status = statusFromUnknown(e);
		if (status !== null) throw error(status, 'Failed to fetch WAD');
		throw error(500, 'Failed to fetch WAD');
	}
};
