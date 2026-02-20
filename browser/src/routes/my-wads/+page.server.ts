import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { WadDraft, UserWad } from '$lib/types/wadinfo';

export const load: PageServerLoad = async ({ cookies, fetch, request }) => {
	const accessToken = cookies.get('dorch_access_token');

	if (!accessToken) {
		return {
			drafts: [] as WadDraft[],
			publishedWads: [] as UserWad[],
			notAuthenticated: true
		};
	}

	const forwardedFor = request.headers.get('x-forwarded-for') ?? undefined;

	try {
		const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });
		// Fetch both drafts and published WADs in parallel
		const [draftsResponse, wadsResponse] = await Promise.all([
			wadinfo.listDrafts(),
			wadinfo.listUserWads()
		]);
		return {
			drafts: draftsResponse.items,
			publishedWads: wadsResponse.items,
			notAuthenticated: false
		};
	} catch (err) {
		console.error('Failed to load WADs:', err);
		const status = (err as { status?: number })?.status;
		if (status === 401) {
			return {
				drafts: [] as WadDraft[],
				publishedWads: [] as UserWad[],
				notAuthenticated: true
			};
		}
		return {
			drafts: [] as WadDraft[],
			publishedWads: [] as UserWad[],
			notAuthenticated: false,
			loadError: 'Failed to load WADs'
		};
	}
};
