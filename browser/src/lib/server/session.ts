import { dev } from '$app/environment';
import type { Cookies, RequestEvent } from '@sveltejs/kit';
import { createIamClient } from '$lib/server/iam';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

const ACCESS_TOKEN_TTL_SECONDS = 60 * 5;

export const REFRESH_TOKEN_COOKIE = 'dorch_refresh_token';
export const REFRESH_TOKEN_EXP_COOKIE = 'dorch_refresh_token_expires_at';
export const LOGGED_IN_COOKIE = 'dorch_logged_in';
export const USERNAME_COOKIE = 'dorch_username';
export const USER_ID_COOKIE = 'dorch_user_id';
export const ACCESS_TOKEN_COOKIE = 'dorch_access_token';
export const ACCESS_TOKEN_EXP_COOKIE = 'dorch_access_token_expires_at';

export type AuthSession = {
	userId: string;
	username: string | null;
	accessToken: string;
};

export function clearAuthCookies(cookies: Cookies) {
	for (const name of [
		REFRESH_TOKEN_COOKIE,
		REFRESH_TOKEN_EXP_COOKIE,
		ACCESS_TOKEN_COOKIE,
		ACCESS_TOKEN_EXP_COOKIE,
		LOGGED_IN_COOKIE,
		USERNAME_COOKIE,
		USER_ID_COOKIE
	]) {
		cookies.delete(name, {
			path: '/',
			sameSite: 'lax',
			secure: !dev
		});
	}
}

function isAccessTokenFresh(cookies: Cookies): boolean {
	const accessToken = cookies.get(ACCESS_TOKEN_COOKIE);
	const expiresAtRaw = cookies.get(ACCESS_TOKEN_EXP_COOKIE);
	if (!accessToken || !expiresAtRaw) return false;
	const expiresAt = Date.parse(expiresAtRaw);
	if (!Number.isFinite(expiresAt)) return false;
	return Date.now() + 10_000 < expiresAt;
}

function decodeJwtSub(token: string): string | null {
	const parts = token.split('.');
	if (parts.length < 2) return null;
	try {
		const payloadB64 = parts[1].replace(/-/g, '+').replace(/_/g, '/');
		const pad = payloadB64.length % 4;
		const padded = payloadB64 + (pad ? '='.repeat(4 - pad) : '');
		const payloadStr = atob(padded);
		const payload = JSON.parse(payloadStr) as { sub?: unknown };
		return typeof payload.sub === 'string' && payload.sub.length > 0 ? payload.sub : null;
	} catch {
		return null;
	}
}

function writeAccessCookies(cookies: Cookies, accessToken: string) {
	const expiresAt = new Date(Date.now() + ACCESS_TOKEN_TTL_SECONDS * 1000).toISOString();
	cookies.set(ACCESS_TOKEN_COOKIE, accessToken, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		secure: !dev,
		maxAge: ACCESS_TOKEN_TTL_SECONDS
	});
	cookies.set(ACCESS_TOKEN_EXP_COOKIE, expiresAt, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		secure: !dev,
		maxAge: ACCESS_TOKEN_TTL_SECONDS
	});
}

export async function getAuthenticatedSession(event: Pick<RequestEvent, 'cookies' | 'fetch' | 'request'>): Promise<AuthSession | null> {
	const { cookies, fetch, request } = event;
	const usernameCookie = cookies.get(USERNAME_COOKIE) ?? null;
	let userId = cookies.get(USER_ID_COOKIE) ?? null;

	if (isAccessTokenFresh(cookies)) {
		const accessToken = cookies.get(ACCESS_TOKEN_COOKIE);
		if (!accessToken) return null;
		if (!userId) {
			userId = decodeJwtSub(accessToken);
			if (userId) {
				cookies.set(USER_ID_COOKIE, userId, {
					path: '/',
					httpOnly: true,
					sameSite: 'lax',
					secure: !dev
				});
			}
		}
		if (!userId) return null;
		return { userId, username: usernameCookie, accessToken };
	}

	const refreshToken = cookies.get(REFRESH_TOKEN_COOKIE);
	if (!refreshToken) return null;

	try {
		const forwardedFor = getTrustedXForwardedFor(request);
		const iam = createIamClient(fetch, { forwardedFor });
		const creds = await iam.refresh(refreshToken);
		const accessToken = creds?.jwt?.access_token;
		if (!accessToken) return null;

		writeAccessCookies(cookies, accessToken);
		cookies.set(LOGGED_IN_COOKIE, '1', {
			path: '/',
			httpOnly: true,
			sameSite: 'lax',
			secure: !dev
		});
		if (creds.username) {
			cookies.set(USERNAME_COOKIE, creds.username, {
				path: '/',
				httpOnly: true,
				sameSite: 'lax',
				secure: !dev
			});
		}
		if (creds.id) {
			cookies.set(USER_ID_COOKIE, creds.id, {
				path: '/',
				httpOnly: true,
				sameSite: 'lax',
				secure: !dev
			});
			userId = creds.id;
		}
		if (!userId) {
			userId = decodeJwtSub(accessToken);
		}
		if (!userId) return null;
		return {
			userId,
			username: creds.username ?? usernameCookie,
			accessToken
		};
	} catch {
		clearAuthCookies(cookies);
		return null;
	}
}
