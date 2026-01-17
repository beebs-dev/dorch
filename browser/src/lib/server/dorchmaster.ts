import { env } from '$env/dynamic/private';
import type { ListGamesResponse } from '$lib/types/games';

class DorchMasterHttpError extends Error {
	readonly status: number;
	readonly body?: string;

	constructor(message: string, status: number, body?: string) {
		super(message);
		this.name = 'DorchMasterHttpError';
		this.status = status;
		this.body = body;
	}
}

function getBaseUrl(): string {
	const base = env.MASTER_BASE_URL;
	if (!base) {
		throw new Error('Missing required private env var MASTER_BASE_URL');
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
		throw new DorchMasterHttpError(
			`dorch-master request failed: ${res.status} ${res.statusText}`,
			res.status,
			body
		);
	}
	return (await res.json()) as T;
}

export function createDorchMasterClient(fetchFn: typeof fetch) {
	return {
		async listGames(): Promise<ListGamesResponse> {
			return requestJson<ListGamesResponse>(fetchFn, '/game');
		},
		DorchMasterHttpError
	};
}
