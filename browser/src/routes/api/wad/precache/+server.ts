import { json, type RequestHandler } from '@sveltejs/kit';

import { createWadinfoClient } from '$lib/server/wadinfo';
import { getRedisClient } from '$lib/server/redis';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

const DEFAULT_LIMIT = 25;
const DEFAULT_FEATURED_LIMIT = 6;
const NEAR_EXPIRY_SECONDS = 5;

function clampInt(v: number, min: number, max: number): number {
	return Math.max(min, Math.min(max, v));
}

export const POST: RequestHandler = async ({ fetch, request, url }) => {
	const limit = clampInt(Number(url.searchParams.get('limit') ?? DEFAULT_LIMIT), 1, 100);
	const cacheKey = `feat:l=${limit}`;

	let ttlSeconds: number | null = null;
	let refreshed = false;

	try {
		const redis = await getRedisClient();
		ttlSeconds = await redis.ttl(cacheKey);

		const missing = ttlSeconds === -2;
		const hasExpiry = ttlSeconds >= 0;
		const nearExpiry = hasExpiry && ttlSeconds <= NEAR_EXPIRY_SECONDS;

		if (missing || nearExpiry) {
			const forwardedFor = getTrustedXForwardedFor(request);
			const wadinfo = createWadinfoClient(fetch, { forwardedFor });

			await wadinfo.featuredView({
				offset: 0,
				limit,
				desc: false,
				featuredLimit: DEFAULT_FEATURED_LIMIT,
				bypassCache: true
			});

			refreshed = true;
			// Re-check TTL after refresh (best-effort).
			try {
				ttlSeconds = await redis.ttl(cacheKey);
			} catch {
				// ignore
			}
		}
	} catch (e) {
		// Non-fatal endpoint; caller is expected to fire-and-forget.
		return json(
			{
				ok: false,
				reason: e instanceof Error ? e.message : 'failed',
				cacheKey,
				ttlSeconds,
				refreshed
			} as const,
			{ headers: { 'cache-control': 'private, max-age=0' } }
		);
	}

	return json(
		{
			ok: true,
			cacheKey,
			ttlSeconds,
			refreshed
		} as const,
		{ headers: { 'cache-control': 'private, max-age=0' } }
	);
};
