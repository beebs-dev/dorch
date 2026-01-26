import { error, json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import type { WadMeta } from '$lib/types/wadinfo';

type WadMetaOk = { id: string; ok: true; meta: WadMeta };
type WadMetaErr = { id: string; ok: false; status?: number; error: string };
export type WadMetaLookupItem = WadMetaOk | WadMetaErr;
export type WadMetaLookupResponse = { items: WadMetaLookupItem[] };

function uniquePreserveOrder(items: string[]): string[] {
	const seen = new Set<string>();
	const out: string[] = [];
	for (const item of items) {
		const key = item.trim().toLowerCase();
		if (!key) continue;
		if (seen.has(key)) continue;
		seen.add(key);
		out.push(item.trim());
	}
	return out;
}

function parseIds(url: URL): string[] {
	const ids: string[] = [];
	for (const v of url.searchParams.getAll('id')) ids.push(v);
	const joined = url.searchParams.get('ids');
	if (joined) ids.push(...joined.split(','));
	return uniquePreserveOrder(ids.map((s) => s.trim()).filter(Boolean));
}

function statusFromUnknown(e: unknown): number | undefined {
	if (!e || typeof e !== 'object') return undefined;
	const anyE = e as { status?: unknown };
	return typeof anyE.status === 'number' ? anyE.status : undefined;
}

export const GET: RequestHandler = async ({ fetch, url, request }) => {
	const ids = parseIds(url);
	if (ids.length === 0) throw error(400, 'Missing query param: id or ids');
	if (ids.length > 25) throw error(400, 'Too many IDs (max 25)');

	const forwardedFor = getTrustedXForwardedFor(request);
	const wadinfo = createWadinfoClient(fetch, { forwardedFor });

	const items: WadMetaLookupItem[] = await Promise.all(
		ids.map(async (id): Promise<WadMetaLookupItem> => {
			try {
				const wad = await wadinfo.getWad(id);
				return { id, ok: true, meta: wad.meta };
			} catch (e) {
				const status = statusFromUnknown(e);
				const msg =
					status === 404
						? 'Not found'
						: status
							? 'Failed to fetch'
							: 'Failed to fetch';
				return { id, ok: false, status, error: msg };
			}
		})
	);

	return json({ items } satisfies WadMetaLookupResponse, {
		headers: {
			// Avoid caching personalized / varying results.
			'cache-control': 'private, max-age=0'
		}
	});
};
