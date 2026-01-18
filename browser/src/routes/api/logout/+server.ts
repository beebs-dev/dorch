import { dev } from '$app/environment';
import { json, type RequestHandler } from '@sveltejs/kit';

const REFRESH_TOKEN_COOKIE = 'dorch_refresh_token';
const REFRESH_TOKEN_EXP_COOKIE = 'dorch_refresh_token_expires_at';
const LOGGED_IN_COOKIE = 'dorch_logged_in';

export const POST: RequestHandler = async ({ cookies }) => {
	for (const name of [REFRESH_TOKEN_COOKIE, REFRESH_TOKEN_EXP_COOKIE, LOGGED_IN_COOKIE]) {
		try {
			cookies.delete(name, {
				path: '/',
				sameSite: 'lax',
				secure: !dev
			});
		} catch {
			// ignore
		}
	}

	return json({ ok: true });
};
