import { env } from '$env/dynamic/private';
import type {
	GetWadMapResponse,
	GetWadResponse,
	ListWadsResponse,
	WadMeta,
	WadImage,
	WadSearchResults
} from '$lib/types/wadinfo';

function rewriteS3SpacesUrl(url: string | null | undefined): string | null | undefined {
	if (!url) return url;
	if (!url.startsWith('s3://')) return url;

	// Expected shape: s3://bucketname/key
	const rest = url.slice('s3://'.length);
	const firstSlash = rest.indexOf('/');
	if (firstSlash <= 0) return url;

	const bucket = rest.slice(0, firstSlash);
	let key = rest.slice(firstSlash + 1);
	if (key.startsWith('/')) key = key.slice(1);

	// Dorch uses DigitalOcean Spaces (S3-compatible). The public URL format is:
	// https://{bucket}.nyc3.digitaloceanspaces.com/{key}
	return key.length
		? `https://${bucket}.nyc3.digitaloceanspaces.com/${key}`
		: `https://${bucket}.nyc3.digitaloceanspaces.com/`;
}

function normalizeWadMeta(meta: WadMeta): WadMeta {
	return {
		...meta,
		file: {
			...(meta.file ?? {}),
			url: rewriteS3SpacesUrl(meta.file?.url ?? null) ?? null
		}
	};
}

function normalizeGetWadResponse(wad: GetWadResponse): GetWadResponse {
	return {
		...wad,
		meta: normalizeWadMeta(wad.meta)
	};
}

function normalizeListWadsResponse(res: ListWadsResponse): ListWadsResponse {
	return {
		...res,
		items: (res.items ?? []).map(normalizeWadMeta)
	};
}

function normalizeWadSearchResults(res: WadSearchResults): WadSearchResults {
	return {
		...res,
		items: (res.items ?? []).map(normalizeWadMeta)
	};
}

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

async function requestJson<T>(fetchFn: typeof fetch, path: string, init?: RequestInit): Promise<T> {
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
		async featured(opts: { limit?: number } = {}): Promise<ListWadsResponse> {
			const url = buildUrl('/featured');
			if (typeof opts.limit === 'number') url.searchParams.set('limit', String(opts.limit));
			const res = await requestJson<ListWadsResponse>(
				fetchFn,
				url.pathname + `?${url.searchParams.toString()}`
			);
			return normalizeListWadsResponse(res);
		},

		async listWads(opts: {
			offset: number;
			limit: number;
			desc?: boolean;
		}): Promise<ListWadsResponse> {
			const url = buildUrl('/wad');
			url.searchParams.set('offset', String(opts.offset));
			url.searchParams.set('limit', String(opts.limit));
			if (opts.desc) url.searchParams.set('d', 'true');
			const res = await requestJson<ListWadsResponse>(
				fetchFn,
				url.pathname + `?${url.searchParams.toString()}`
			);
			return normalizeListWadsResponse(res);
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
			const res = await requestJson<WadSearchResults>(
				fetchFn,
				url.pathname + `?${url.searchParams.toString()}`
			);
			return normalizeWadSearchResults(res);
		},

		async getWad(wadId: string): Promise<GetWadResponse> {
			const wad = await requestJson<GetWadResponse>(fetchFn, `/wad/${encodeURIComponent(wadId)}`);
			return normalizeGetWadResponse(wad);
		},

		async getWadMap(wadId: string, mapName: string): Promise<GetWadMapResponse> {
			// Note: backend route is singular `/wad/{id}/map/{map}`.
			const res = await requestJson<GetWadMapResponse>(
				fetchFn,
				`/wad/${encodeURIComponent(wadId)}/map/${encodeURIComponent(mapName)}`
			);
			return {
				...res,
				wad_meta: normalizeWadMeta(res.wad_meta)
			};
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
