import { env } from '$env/dynamic/private';

import { createClient, type RedisClientType } from 'redis';

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

let redisClient: RedisClientType | null = null;
let redisConnecting: Promise<RedisClientType> | null = null;

let shuttingDown = false;

export async function shutdownRedis(): Promise<void> {
	shuttingDown = true;
	redisConnecting = null;

	const client = redisClient;
	redisClient = null;
	if (!client) return;

	try {
		// Disconnect immediately (do not wait for pending commands like `quit()`).
		await client.disconnect();
	} catch {
		// Best-effort; shutdown should not hang on Redis.
	}
}

// Ensure Redis doesn't keep the event loop alive during shutdown.
const globalAny = globalThis as any;
if (
	typeof process !== 'undefined' &&
	typeof process.once === 'function' &&
	!globalAny.__dorchRedisShutdownInstalled
) {
	globalAny.__dorchRedisShutdownInstalled = true;
	process.once('SIGTERM', () => {
		void shutdownRedis();
	});
	process.once('SIGINT', () => {
		void shutdownRedis();
	});
}

export async function getRedisClient(): Promise<RedisClientType> {
	if (shuttingDown) {
		throw new Error('redis is shutting down');
	}
	if (redisClient?.isOpen) return redisClient;
	if (redisConnecting) return redisConnecting;

	if (!redisClient) {
		const url = buildRedisUrl();
		redisClient = createClient({
			url,
			socket: {
				reconnectStrategy: (retries: number) => {
					if (shuttingDown) return new Error('shutting down');
					return Math.min(50 * (retries + 1), 2_000);
				}
			}
		});
		redisClient.on('error', (e) => {
			console.error('redis error', e);
		});
	}

	redisConnecting = (async () => {
		try {
			if (!redisClient) throw new Error('redis client missing');
			if (!redisClient.isOpen) {
				await redisClient.connect();
			}
			return redisClient;
		} catch (e) {
			redisClient = null;
			throw e;
		} finally {
			redisConnecting = null;
		}
	})();

	return redisConnecting;
}
