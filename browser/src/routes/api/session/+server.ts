import { ACCESS_TOKEN_EXP_COOKIE, getAuthenticatedSession } from '$lib/server/session';
import { json, type RequestHandler } from '@sveltejs/kit';

export const GET: RequestHandler = async (event) => {
	const session = await getAuthenticatedSession(event);
	if (!session) {
		return json({ error: 'Not authenticated' }, { status: 401 });
	}

	return json(
		{
			userId: session.userId,
			username: session.username,
			accessTokenExpiresAt: event.cookies.get(ACCESS_TOKEN_EXP_COOKIE) ?? null
		},
		{
			headers: {
				'cache-control': 'private, no-store'
			}
		}
	);
};
