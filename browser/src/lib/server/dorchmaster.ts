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

export type HomeResponse = {
	games: ListGamesResponse;
	jumbotron: JumbotronResponse;
};

export type CreateGameRequest = {
	name: string;
	iwad: string;
	user_ids: string[];
	private?: boolean;
	warp?: string;
	skill?: number;
	dmflags?: number;
	frag_limit?: number;
	time_limit?: number;
	motd?: string;
	files?: string[];
	max_players?: number;
};

export type CreateGameResponse = {
	game_id: string;
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

function normalizeBaseUrl(base: string): string {
	return base.endsWith('/') ? base : `${base}/`;
}

function getInternalBaseUrl(): string {
	const base = env.MASTER_BASE_URL;
	if (!base) throw new Error('Missing required private env var MASTER_BASE_URL');
	return normalizeBaseUrl(base);
}

function getPublicBaseUrl(): string {
	// In-cluster this may differ (public port/proxy). For local dev we allow falling back.
	const base = env.MASTER_PUBLIC_BASE_URL ?? env.MASTER_BASE_URL;
	if (!base) {
		throw new Error(
			'Missing required private env var MASTER_PUBLIC_BASE_URL (or fallback MASTER_BASE_URL)'
		);
	}
	return normalizeBaseUrl(base);
}

function buildUrl(baseUrl: string, path: string): URL {
	return new URL(path.replace(/^\//, ''), baseUrl);
}

async function requestJson<T>(
	fetchFn: typeof fetch,
	baseUrl: string,
	path: string,
	init?: RequestInit,
	opts?: { forwardedFor?: string; bearerToken?: string }
): Promise<T> {
	const url = buildUrl(baseUrl, path);
	const headers = new Headers(init?.headers);
	if (!headers.has('accept')) headers.set('accept', 'application/json');
	if (opts?.forwardedFor && !headers.has('x-forwarded-for')) {
		headers.set('x-forwarded-for', opts.forwardedFor);
	}
	if (opts?.bearerToken && !headers.has('authorization')) {
		headers.set('authorization', `Bearer ${opts.bearerToken}`);
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

export function createDorchMasterClient(
	fetchFn: typeof fetch,
	opts?: { forwardedFor?: string; bearerToken?: string; baseUrl?: string }
) {
	const forwardedFor = opts?.forwardedFor;
	const bearerToken = opts?.bearerToken;
	const baseUrl = opts?.baseUrl ?? getInternalBaseUrl();
	return {
		async listGames(): Promise<ListGamesResponse> {
			return requestJson<ListGamesResponse>(fetchFn, baseUrl, '/game', undefined, { forwardedFor });
		},
		async getGame(gameId: string): Promise<GameSummary> {
			return requestJson<GameSummary>(
				fetchFn,
				baseUrl,
				`/game/${encodeURIComponent(gameId)}`,
				undefined,
				{ forwardedFor }
			);
		},
		async getJumbotron(): Promise<JumbotronResponse> {
			// dorch-master returns: { items: [{ game_id, url }, ...] }
			return requestJson<JumbotronResponse>(fetchFn, baseUrl, '/jumbotron', undefined, { forwardedFor });
		},
		async getHome(): Promise<HomeResponse> {
			return requestJson<HomeResponse>(fetchFn, baseUrl, '/home', undefined, { forwardedFor });
		},
		async listMyGames(): Promise<ListGamesResponse> {
			return requestJson<ListGamesResponse>(fetchFn, baseUrl, '/my/games', undefined, {
				forwardedFor,
				bearerToken
			});
		},
		async createGame(payload: CreateGameRequest): Promise<CreateGameResponse> {
			return requestJson<CreateGameResponse>(
				fetchFn,
				baseUrl,
				'/game',
				{
					method: 'POST',
					headers: {
						'content-type': 'application/json'
					},
					body: JSON.stringify(payload)
				},
				{ forwardedFor, bearerToken }
			);
		},
		DorchMasterHttpError
	};
}

export function createDorchMasterPublicClient(
	fetchFn: typeof fetch,
	opts?: { forwardedFor?: string; bearerToken?: string }
) {
	return createDorchMasterClient(fetchFn, {
		...opts,
		baseUrl: getPublicBaseUrl()
	});
}
