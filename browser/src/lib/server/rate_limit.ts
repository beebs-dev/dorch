import type { RequestEvent } from '@sveltejs/kit';

import { env } from '$env/dynamic/private';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

import { readFileSync } from 'node:fs';
import { createClient, type RedisClientType } from 'redis';

type RateLimiterConfig = {
	burstLimit: number;
	burstWindowMs: number;
	longLimit: number;
	longWindowMs: number;
	maxListSize: number;
	keyPrefix: string;
};

function parseIntEnv(name: string, defaultValue: number): number {
	const raw = env[name];
	if (!raw) return defaultValue;
	const parsed = Number.parseInt(raw, 10);
	return Number.isFinite(parsed) ? parsed : defaultValue;
}

function getRateLimiterConfig(): RateLimiterConfig {
	return {
		burstLimit: parseIntEnv('RATE_LIMITER_BURST_LIMIT', 20),
		burstWindowMs: parseIntEnv('RATE_LIMITER_BURST_WINDOW_MS', 5_000),
		longLimit: parseIntEnv('RATE_LIMITER_LONG_LIMIT', 200),
		longWindowMs: parseIntEnv('RATE_LIMITER_LONG_WINDOW_MS', 60_000),
		maxListSize: parseIntEnv('RATE_LIMITER_MAX_LIST_SIZE', 512),
		keyPrefix: env.RATE_LIMITER_KEY_PREFIX ?? 'rate:',
	};
}

function buildRedisUrl(): string {
	const proto = (env.REDIS_PROTO && env.REDIS_PROTO.length ? env.REDIS_PROTO : 'redis').trim();
	const host = (env.REDIS_HOST ?? '127.0.0.1').trim();
	const port = (env.REDIS_PORT ?? '6379').trim();
	const username = env.REDIS_USERNAME?.trim();
	const password = env.REDIS_PASSWORD?.trim();

	let auth = '';
	if (username && username.length) {
		auth += encodeURIComponent(username);
		if (password && password.length) {
			auth += `:${encodeURIComponent(password)}`;
		}
		auth += '@';
	} else if (password && password.length) {
		auth += `:${encodeURIComponent(password)}@`;
	}

	return `${proto}://${auth}${host}:${port}/`;
}

let luaScript: string | null = null;
function getLuaScript(): string {
	if (luaScript) return luaScript;
	const scriptPath = env.RATE_LIMITER_LUA_PATH ?? 'rate_limit.lua';
	luaScript = readFileSync(scriptPath, 'utf8');
	return luaScript;
}

let redisClient: RedisClientType | null = null;
let redisConnecting: Promise<RedisClientType> | null = null;
let scriptSha: string | null = null;

async function getRedisClient(): Promise<RedisClientType> {
	if (redisClient?.isOpen) return redisClient;
	if (redisConnecting) return redisConnecting;

	const url = buildRedisUrl();
	const client = createClient({ url });
	client.on('error', (e) => {
		console.error('redis error', e);
	});

	redisConnecting = (async () => {
		try {
			if (!client.isOpen) {
				await client.connect();
			}
			redisClient = client as any;
			return client;
		} catch (e) {
			redisClient = null;
			scriptSha = null;
			throw e;
		} finally {
			redisConnecting = null;
		}
	})() as any;

	return redisConnecting as Promise<RedisClientType>;
}

async function evalRateLimitScript(listKey: string, args: string[]): Promise<number> {
	const client = await getRedisClient();
	const lua = getLuaScript();

	if (!scriptSha) {
		scriptSha = (await client.sendCommand(['SCRIPT', 'LOAD', lua])) as unknown as string;
	}

	try {
		const result = await client.sendCommand(['EVALSHA', scriptSha, '1', listKey, ...args]);
		return Number(result);
	} catch (e: any) {
		const message = typeof e?.message === 'string' ? e.message : '';
		if (message.includes('NOSCRIPT')) {
			scriptSha = (await client.sendCommand(['SCRIPT', 'LOAD', lua])) as unknown as string;
			const result = await client.sendCommand(['EVALSHA', scriptSha, '1', listKey, ...args]);
			return Number(result);
		}
		throw e;
	}
}

function shouldRateLimitPath(pathname: string): boolean {
	// Skip SvelteKit static assets and common lightweight endpoints
	if (pathname.startsWith('/_app/')) return false;
	if (pathname === '/favicon.ico') return false;
	if (pathname === '/robots.txt') return false;
	return true;
}

function extractClientIp(event: RequestEvent): string | undefined {
	const forwardedFor = getTrustedXForwardedFor(event.request);
	if (!forwardedFor) return undefined;

	const first = forwardedFor.split(',')[0]?.trim();
	if (!first) return undefined;

	// Ignore internal cluster traffic (mirrors common's Rust middleware)
	if (first.startsWith('10.') || first.startsWith('192.168.') || first.startsWith('172.')) {
		return undefined;
	}

	return first;
}

export async function isRateLimited(event: RequestEvent): Promise<boolean> {
	if (!shouldRateLimitPath(event.url.pathname)) return false;

	const ip = extractClientIp(event);
	if (!ip) return false;

	const config = getRateLimiterConfig();
	const listKey = `${config.keyPrefix}ip:${ip}`;

	const nowMs = Date.now();
	const argv = [
		String(config.burstLimit),
		String(config.burstWindowMs),
		String(config.longLimit),
		String(config.longWindowMs),
		String(nowMs),
		String(config.maxListSize),
	];

	try {
		const result = await evalRateLimitScript(listKey, argv);
		return result !== 1;
	} catch (e) {
		// Fail open if Redis/script is unavailable.
		console.error('rate limiter failed; allowing request', e);
		return false;
	}
}