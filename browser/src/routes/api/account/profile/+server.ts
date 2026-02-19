import { json, type RequestHandler } from '@sveltejs/kit';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import { getAuthenticatedSession } from '$lib/server/session';

export const GET: RequestHandler = async (event) => {
	const session = await getAuthenticatedSession(event);
	if (!session) {
		return json({ error: 'Not authenticated' }, { status: 401 });
	}
	const forwardedFor = getTrustedXForwardedFor(event.request);
	const wadinfo = createWadinfoClient(event.fetch, {
		forwardedFor,
		bearerToken: session.accessToken
	});

	try {
		const profile = await wadinfo.getUserProfile(session.userId);
		return json(profile, {
			headers: {
				'cache-control': 'private, max-age=0'
			}
		});
	} catch (err: any) {
		const status = typeof err?.status === 'number' ? err.status : 500;
		const msg = status === 404 ? 'Profile not found' : 'Failed to load profile';
		return json({ error: msg }, { status });
	}
};
