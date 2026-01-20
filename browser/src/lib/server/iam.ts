import { env } from '$env/dynamic/private';
import type { UserCredentials } from '$lib/types/auth';

class IamHttpError extends Error {
	readonly status: number;
	readonly body?: string;

	constructor(message: string, status: number, body?: string) {
		super(message);
		this.name = 'IamHttpError';
		this.status = status;
		this.body = body;
	}
}

function getBaseUrl(): string {
	const base = env.IAM_BASE_URL;
	if (!base) {
		throw new Error('Missing required private env var IAM_BASE_URL');
	}
	return base.endsWith('/') ? base : `${base}/`;
}

function buildUrl(path: string): URL {
	const base = getBaseUrl();
	return new URL(path.replace(/^\//, ''), base);
}

function getLoginPath(): string {
	const configured = env.IAM_LOGIN_PATH;
	if (!configured) return '/user/login';
	return configured.startsWith('/') ? configured : `/${configured}`;
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
		throw new IamHttpError(`iam request failed: ${res.status} ${res.statusText}`, res.status, body);
	}
	return (await res.json()) as T;
}

export function createIamClient(fetchFn: typeof fetch, opts?: { forwardedFor?: string }) {
	const forwardedFor = opts?.forwardedFor;
	return {
		async login(username: string, password: string): Promise<UserCredentials> {
			return requestJson<UserCredentials>(
				fetchFn,
				getLoginPath(),
				{
				method: 'POST',
				headers: {
					'content-type': 'application/json'
				},
				body: JSON.stringify({ username, password })
				},
				{ forwardedFor }
			);
		},
		IamHttpError
	};
}
