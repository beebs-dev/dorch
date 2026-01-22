import { env } from '$env/dynamic/private';
import type { GameSummary, ListGamesResponse } from '$lib/types/games';

export type JumbotronItem = {
	game_id: string;
	// Back-compat: older dorch-master responses provide a single HLS `url`.
	// Newer responses may provide `hls` / `rtc` + `thumbnail`.
	url?: string;
	hls?: string;
	rtc?: string;
	thumbnail?: string;
	name?: string;
	player_count?: number;
	max_players?: number;
	monster_kill_count?: number;
	monster_total?: number;
};

export type JumbotronResponse = {
	items: JumbotronItem[];
};

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

async function requestJson<T>(
	fetchFn: typeof fetch,
	path: string,
	init?: RequestInit,
	opts?: { forwardedFor?: string }
): Promise<T> {
	const url = buildUrl(path);
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
		throw new DorchMasterHttpError(
			`dorch-master request failed: ${res.status} ${res.statusText}`,
			res.status,
			body
		);
	}
	return (await res.json()) as T;
}

export function createDorchMasterClient(fetchFn: typeof fetch, opts?: { forwardedFor?: string }) {
	const forwardedFor = opts?.forwardedFor;
	return {
		async listGames(): Promise<ListGamesResponse> {
			return requestJson<ListGamesResponse>(fetchFn, '/game', undefined, { forwardedFor });
		},
		async getGame(gameId: string): Promise<GameSummary> {
			return requestJson<GameSummary>(
				fetchFn,
				`/game/${encodeURIComponent(gameId)}`,
				undefined,
				{ forwardedFor }
			);
		},
		async getJumbotron(): Promise<JumbotronResponse> {
			// dorch-master returns: { items: [{ game_id, url }, ...] }
			return requestJson<JumbotronResponse>(fetchFn, '/jumbotron', undefined, { forwardedFor });
		},
		DorchMasterHttpError
	};
}
