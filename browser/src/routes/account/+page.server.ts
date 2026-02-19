import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { UserProfileFull } from '$lib/types/wadinfo';

export const load: PageServerLoad = async ({ cookies, fetch, request }) => {
	const accessToken = cookies.get('dorch_access_token');
	const userId = cookies.get('dorch_user_id');

	if (!accessToken || !userId) {
		return {
			profile: null,
			notAuthenticated: true
		};
	}

	const forwardedFor = request.headers.get('x-forwarded-for') ?? undefined;

	try {
		const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });
		const profile = await wadinfo.getUserProfile(userId) as UserProfileFull;
		return {
			profile,
			notAuthenticated: false
		};
	} catch (err) {
		console.error('Failed to load user profile:', err);
		// If we get a 401, the token is invalid/expired
		const status = (err as { status?: number })?.status;
		if (status === 401) {
			return {
				profile: null,
				notAuthenticated: true
			};
		}
		return {
			profile: null,
			notAuthenticated: false,
			loadError: 'Failed to load profile'
		};
	}
};
