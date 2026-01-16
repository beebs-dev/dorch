import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { WadImage, WadMeta } from '$lib/types/wadinfo';
import { clampInt, getIntParam, getStringParam } from '$lib/utils/format';

type WadBrowserResults = {
	items: WadMeta[];
	full_count: number;
	offset: number;
	limit: number;
	truncated: boolean;
	request_id?: string;
	query?: string;
};

export const load: PageServerLoad = async ({ fetch, url, setHeaders }) => {
	const q = getStringParam(url, 'q');
	const sort = url.searchParams.get('sort') ?? 'featured';
	const offset = clampInt(getIntParam(url, 'offset') ?? 0, 0, 1_000_000_000);
	const limit = clampInt(getIntParam(url, 'limit') ?? 25, 1, 100);

	const wadinfo = createWadinfoClient(fetch);

	let results: WadBrowserResults;
	if (q) {
		const resp = await wadinfo.search({ query: q, offset, limit });
		results = {
			items: resp.items,
			full_count: resp.full_count,
			offset: resp.offset,
			limit: resp.limit,
			truncated: resp.truncated,
			request_id: resp.request_id,
			query: resp.query
		};
	} else {
		// wadinfo currently supports alphabetical sorting only (ascending/descending).
		// Keep `sort` values stable so we can map to future backend sorting.
		const desc = sort === 'alphabetical_desc';
		const resp = await wadinfo.listWads({ offset, limit, desc });
		results = resp;
	}

	// Short TTL; safe for SSR and avoids stale UIs without needing client fetch.
	setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=10' });

	let featured: Array<{ wad: WadMeta; images: WadImage[] }> = [];
	if (!q && offset === 0) {
		const slice = (await wadinfo.featured({ limit: 6 })).items;
		const images = await Promise.all(
			slice.map(async (wad) => {
				try {
					const detail = await wadinfo.getWad(wad.id);
					return (detail.maps ?? []).flatMap((m) => m.images ?? []);
				} catch {
					return [];
				}
			})
		);
		featured = slice.map((wad, i) => {
			const all = images[i] ?? [];
			const nonPano = all.filter((img) => (img.type ?? img.kind) !== 'pano');
			// Featured thumbnails should never use pano renders.
			// If a WAD has only panos, we intentionally show the placeholder.
			return { wad, images: nonPano };
		});
	}

	return {
		q,
		sort,
		offset,
		limit,
		results,
		featured
	};
};
