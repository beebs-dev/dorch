import { error, json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import type { WadMeta } from '$lib/types/wadinfo';

type WadSearchMeta = Pick<WadMeta, 'id' | 'title' | 'filename' | 'filenames'> & {
	file: { type?: string; size?: number | null };
};

export type WadSearchHit = {
	id: string;
	meta: WadSearchMeta;
};

export type WadSearchResponse = {
	items: WadSearchHit[];
	request_id?: string;
	query: string;
};

function clampInt(v: number, min: number, max: number): number {
	return Math.max(min, Math.min(max, v));
}

export const GET: RequestHandler = async ({ fetch, url, request }) => {
	const q = (url.searchParams.get('q') ?? '').trim();
	if (!q) throw error(400, 'Missing query param: q');

	const limit = clampInt(Number(url.searchParams.get('limit') ?? 12), 1, 25);
	const offset = clampInt(Number(url.searchParams.get('offset') ?? 0), 0, 10_000);

	const forwardedFor = getTrustedXForwardedFor(request);
	const wadinfo = createWadinfoClient(fetch, { forwardedFor });

	const res = await wadinfo.search({ query: q, offset, limit });
	return json(
		{
			items: (res.items ?? []).map((m) => ({
				id: m.id,
				meta: {
					id: m.id,
					title: m.title ?? null,
					filename: m.filename ?? null,
					filenames: m.filenames ?? null,
					file: {
						type: m.file?.type,
						size: m.file?.size ?? null
					}
				}
			})),
			request_id: res.request_id,
			query: res.query
		} satisfies WadSearchResponse,
		{
			headers: {
				'cache-control': 'private, max-age=0'
			}
		}
	);
};
