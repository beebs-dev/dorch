import { env } from '$env/dynamic/private';
import type {
	GetWadMapResponse,
	GetWadResponse,
	ListWadsResponse,
	WadImage,
	WadSearchResults
} from '$lib/types/wadinfo';

class WadinfoHttpError extends Error {
	readonly status: number;
	readonly body?: string;

	constructor(message: string, status: number, body?: string) {
		super(message);
		this.name = 'WadinfoHttpError';
		this.status = status;
		this.body = body;
	}
}

function getBaseUrl(): string {
	const base = env.WADINFO_BASE_URL;
	if (!base) {
		throw new Error('Missing required private env var WADINFO_BASE_URL');
	}
	return base.endsWith('/') ? base : `${base}/`;
}

function buildUrl(path: string): URL {
	const base = getBaseUrl();
	return new URL(path.replace(/^\//, ''), base);
}

async function requestJson<T>(
	fetchFn: typeof fetch,
	path: string,
	init?: RequestInit
): Promise<T> {
	const url = buildUrl(path);
	const res = await fetchFn(url, {
		...init,
		headers: {
			accept: 'application/json',
			...(init?.headers ?? {})
		}
	});
	if (!res.ok) {
		let body: string | undefined;
		try {
			body = await res.text();
		} catch {
			// ignore
		}
		throw new WadinfoHttpError(
			`wadinfo request failed: ${res.status} ${res.statusText}`,
			res.status,
			body
		);
	}
	return (await res.json()) as T;
}

export function createWadinfoClient(fetchFn: typeof fetch) {
	return {
		async listWads(opts: { offset: number; limit: number; desc?: boolean }): Promise<ListWadsResponse> {
			const url = buildUrl('/wad');
			url.searchParams.set('offset', String(opts.offset));
			url.searchParams.set('limit', String(opts.limit));
			if (opts.desc) url.searchParams.set('d', 'true');
			return requestJson<ListWadsResponse>(fetchFn, url.pathname + `?${url.searchParams.toString()}`);
		},

		async search(opts: {
			query: string;
			offset: number;
			limit: number;
		}): Promise<WadSearchResults> {
			const url = buildUrl('/search');
			url.searchParams.set('query', opts.query);
			url.searchParams.set('offset', String(opts.offset));
			url.searchParams.set('limit', String(opts.limit));
			return requestJson<WadSearchResults>(fetchFn, url.pathname + `?${url.searchParams.toString()}`);
		},

		async getWad(wadId: string): Promise<GetWadResponse> {
			return requestJson<GetWadResponse>(fetchFn, `/wad/${encodeURIComponent(wadId)}`);
		},

		async getWadMap(wadId: string, mapName: string): Promise<GetWadMapResponse> {
			// Note: backend route is singular `/wad/{id}/map/{map}`.
			return requestJson<GetWadMapResponse>(
				fetchFn,
				`/wad/${encodeURIComponent(wadId)}/map/${encodeURIComponent(mapName)}`
			);
		},

		async listWadMapImages(wadId: string, mapName: string): Promise<WadImage[]> {
			return requestJson<WadImage[]>(
				fetchFn,
				`/wad/${encodeURIComponent(wadId)}/maps/${encodeURIComponent(mapName)}/images`
			);
		},

		WadinfoHttpError
	};
}
