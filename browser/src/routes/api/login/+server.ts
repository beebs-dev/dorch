import { dev } from '$app/environment';
import { createIamClient } from '$lib/server/iam';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import { json, type RequestHandler } from '@sveltejs/kit';

const REFRESH_TOKEN_COOKIE = 'dorch_refresh_token';
const REFRESH_TOKEN_EXP_COOKIE = 'dorch_refresh_token_expires_at';
const LOGGED_IN_COOKIE = 'dorch_logged_in';
const USERNAME_COOKIE = 'dorch_username';
const ACCESS_TOKEN_COOKIE = 'dorch_access_token';
const ACCESS_TOKEN_EXP_COOKIE = 'dorch_access_token_expires_at';

const ACCESS_TOKEN_TTL_SECONDS = 60 * 5;

export const POST: RequestHandler = async ({ request, fetch, cookies }) => {
	const forwardedFor = getTrustedXForwardedFor(request);
	let payload: unknown;
	try {
		payload = await request.json();
	} catch {
		return json({ error: 'Invalid JSON body' }, { status: 400 });
	}

	const username = (payload as any)?.username;
	const password = (payload as any)?.password;
	const rememberMe = (payload as any)?.rememberMe;
	if (typeof username !== 'string' || typeof password !== 'string') {
		return json({ error: 'Missing username/password' }, { status: 400 });
	}

	const trimmedUsername = username.trim();
	if (!trimmedUsername || !password) {
		return json({ error: 'Missing username/password' }, { status: 400 });
	}

	try {
		const iam = createIamClient(fetch, { forwardedFor });
		const creds = await iam.login(trimmedUsername, password);

		const accessToken = creds?.jwt?.access_token ?? null;
		if (typeof accessToken === 'string' && accessToken.length > 0) {
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

		const refreshToken = creds?.jwt?.refresh_token ?? null;
		const refreshExpiresIn = creds?.jwt?.refresh_expires_in ?? null;

		// Mark session as authenticated for server-rendered navigation.
		// This is intentionally independent of refresh-token parsing so the UI can update reliably.
		cookies.set(LOGGED_IN_COOKIE, '1', {
			path: '/',
			httpOnly: true,
			sameSite: 'lax',
			secure: !dev
		});

		if (typeof creds?.username === 'string' && creds.username.length > 0) {
			cookies.set(USERNAME_COOKIE, creds.username, {
				path: '/',
				httpOnly: true,
				sameSite: 'lax',
				secure: !dev
			});
		}

		if (refreshToken) {
			const persist = typeof rememberMe === 'boolean' ? rememberMe : true;
			const maxAge =
				persist && typeof refreshExpiresIn === 'number' ? refreshExpiresIn : undefined;
			const expiresAt =
				typeof refreshExpiresIn === 'number'
					? new Date(Date.now() + refreshExpiresIn * 1000).toISOString()
					: undefined;

			cookies.set(REFRESH_TOKEN_COOKIE, refreshToken, {
				path: '/',
				httpOnly: true,
				sameSite: 'lax',
				secure: !dev,
				...(typeof maxAge === 'number' ? { maxAge } : {})
			});

			if (expiresAt) {
				cookies.set(REFRESH_TOKEN_EXP_COOKIE, expiresAt, {
					path: '/',
					httpOnly: true,
					sameSite: 'lax',
					secure: !dev,
					...(typeof maxAge === 'number' ? { maxAge } : {})
				});
			}
		}

		return json(creds);
	} catch (err: any) {
		const status = typeof err?.status === 'number' ? err.status : 500;
		// Avoid leaking backend internals by default.
		const message =
			status === 401 || status === 403
				? 'Invalid username or password'
				: status === 404
					? 'Login endpoint not found (check IAM_BASE_URL/IAM_LOGIN_PATH)'
					: 'Login failed';
		return json({ error: message }, { status });
	}
};
