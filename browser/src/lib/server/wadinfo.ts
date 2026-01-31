import { env } from '$env/dynamic/private';
import type {
	GetWadMapResponse,
	GetWadResponse,
	FeaturedViewResponse,
	ListWadsResponse,
	MapReference,
	MapThumbnail,
	ResolveMapThumbnailsResponse,
	WadMeta,
	WadImage,
	WadSearchResults
} from '$lib/types/wadinfo';

import { getRedisClient } from '$lib/server/redis';

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

function normalizeFeaturedViewResponse(res: FeaturedViewResponse): FeaturedViewResponse {
	return {
		...res,
		results: normalizeListWadsResponse(res.results),
		featured: (res.featured ?? []).map((it) => ({
			...it,
			wad: normalizeWadMeta(it.wad),
			images: (it.images ?? []) as WadImage[]
		}))
	};
}

function normalizeMapThumbnail(t: MapThumbnail): MapThumbnail {
	return {
		...t,
		url: rewriteS3SpacesUrl(t.url) ?? t.url
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

function parseIntEnv(name: string, defaultValue: number): number {
	const raw = env[name];
	if (!raw) return defaultValue;
	const parsed = Number.parseInt(raw, 10);
	return Number.isFinite(parsed) ? parsed : defaultValue;
}

function shouldUseFeaturedCache(opts: {
	offset: number;
	limit: number;
	desc?: boolean;
	featuredLimit?: number;
}): boolean {
	// The /featured payload includes a dynamic results list; only cache the common
	// “first page, default sort, includes featured section” request.
	return opts.offset === 0 && !opts.desc && (opts.featuredLimit ?? 0) > 0;
}

function getBaseUrl(): string {
	const base = env.WADINFO_BASE_URL;
	if (!base) {
		throw new Error('Missing required private env var WADINFO_BASE_URL');
	}
	return base.endsWith('/') ? base : `${base}/`;
}

function getInternalBaseUrl(): string {
	// Some endpoints (like POST /thumbnails) are only exposed on wadinfo's internal router.
	// Allow the browser SSR server to target that service if configured.
	const base = env.WADINFO_INTERNAL_BASE_URL ?? env.WADINFO_BASE_URL;
	if (!base) {
		throw new Error('Missing required private env var WADINFO_BASE_URL');
	}
	return base.endsWith('/') ? base : `${base}/`;
}

function buildUrl(path: string, opts?: { internal?: boolean }): URL {
	const base = opts?.internal ? getInternalBaseUrl() : getBaseUrl();
	return new URL(path.replace(/^\//, ''), base);
}

async function requestJson<T>(
	fetchFn: typeof fetch,
	path: string,
	init?: RequestInit,
	opts?: { internal?: boolean; forwardedFor?: string }
): Promise<T> {
	const url = buildUrl(path, opts);
	const headers = new Headers(init?.headers);
	if (!headers.has('accept')) headers.set('accept', 'application/json');
	if (opts?.forwardedFor && !headers.has('x-forwarded-for')) {
		headers.set('x-forwarded-for', opts.forwardedFor);
	}
	const res = await fetchFn(url, {
		...init,
		headers
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

export function createWadinfoClient(fetchFn: typeof fetch, opts?: { forwardedFor?: string }) {
	const forwardedFor = opts?.forwardedFor;
	return {
		async featuredView(opts: {
			offset: number;
			limit: number;
			desc?: boolean;
			featuredLimit?: number;
		}): Promise<FeaturedViewResponse> {
			const useCache = shouldUseFeaturedCache(opts);
			const cacheKey = `feat:l=${opts.limit}`;

			if (useCache) {
				try {
					const redis = await getRedisClient();
					const cached = await redis.get(cacheKey);
					if (cached) {
						try {
							const parsed = JSON.parse(cached) as FeaturedViewResponse;
							return normalizeFeaturedViewResponse(parsed);
						} catch {
							// Ignore bad cache entries.
						}
					}
				} catch {
					// Fail open if Redis is unavailable.
				}
			}

			const url = buildUrl('/featured');
			url.searchParams.set('offset', String(opts.offset));
			url.searchParams.set('limit', String(opts.limit));
			if (opts.desc) url.searchParams.set('d', 'true');
			if (typeof opts.featuredLimit === 'number') {
				url.searchParams.set('featured_limit', String(opts.featuredLimit));
			}
			const res = await requestJson<FeaturedViewResponse>(
				fetchFn,
				url.pathname + `?${url.searchParams.toString()}`,
				undefined,
				{ forwardedFor }
			);
			const normalized = normalizeFeaturedViewResponse(res);

			if (useCache) {
				const ttlSeconds = Math.max(1, parseIntEnv('FEATURED_CACHE_TTL', 15));
				try {
					const redis = await getRedisClient();
					await redis.set(cacheKey, JSON.stringify(normalized), { EX: ttlSeconds });
				} catch {
					// Fail open if Redis is unavailable.
				}
			}

			return normalized;
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
				url.pathname + `?${url.searchParams.toString()}`,
				undefined,
				{ forwardedFor }
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
				url.pathname + `?${url.searchParams.toString()}`,
				undefined,
				{ forwardedFor }
			);
			return normalizeWadSearchResults(res);
		},

		async getWad(wadId: string): Promise<GetWadResponse> {
			const wad = await requestJson<GetWadResponse>(
				fetchFn,
				`/wad/${encodeURIComponent(wadId)}`,
				undefined,
				{ forwardedFor }
			);
			return normalizeGetWadResponse(wad);
		},

		async getWadMap(wadId: string, mapName: string): Promise<GetWadMapResponse> {
			// Note: backend route is singular `/wad/{id}/map/{map}`.
			const res = await requestJson<GetWadMapResponse>(
				fetchFn,
				`/wad/${encodeURIComponent(wadId)}/map/${encodeURIComponent(mapName)}`,
				undefined,
				{ forwardedFor }
			);
			return {
				...res,
				wad_meta: normalizeWadMeta(res.wad_meta)
			};
		},

		async listWadMapImages(wadId: string, mapName: string): Promise<WadImage[]> {
			return requestJson<WadImage[]>(
				fetchFn,
				`/wad/${encodeURIComponent(wadId)}/maps/${encodeURIComponent(mapName)}/images`,
				undefined,
				{ forwardedFor }
			);
		},

		async resolveMapThumbnails(items: MapReference[]): Promise<MapThumbnail[]> {
			console.log('Requesting map thumbnails from wadinfo:', { items });
			const res = await requestJson<ResolveMapThumbnailsResponse>(
				fetchFn,
				'/thumbnails',
				{
					method: 'POST',
					headers: {
						'content-type': 'application/json'
					},
					body: JSON.stringify({ items })
				},
				{ internal: true, forwardedFor }
			);
			console.log('Received map thumbnails from wadinfo:', res);
			return (res.items ?? []).map(normalizeMapThumbnail);
		},

		WadinfoHttpError
	};
}
