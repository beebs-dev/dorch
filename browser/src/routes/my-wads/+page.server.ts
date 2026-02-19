import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { WadDraft } from '$lib/types/wadinfo';

export const load: PageServerLoad = async ({ cookies, fetch, request }) => {
	const accessToken = cookies.get('dorch_access_token');

	if (!accessToken) {
		return {
			drafts: [] as WadDraft[],
			notAuthenticated: true
		};
	}

	const forwardedFor = request.headers.get('x-forwarded-for') ?? undefined;

	try {
		const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });
		const response = await wadinfo.listDrafts();
		return {
			drafts: response.items,
			notAuthenticated: false
		};
	} catch (err) {
		console.error('Failed to load drafts:', err);
		const status = (err as { status?: number })?.status;
		if (status === 401) {
			return {
				drafts: [] as WadDraft[],
				notAuthenticated: true
			};
		}
		return {
			drafts: [] as WadDraft[],
			notAuthenticated: false,
			loadError: 'Failed to load WADs'
		};
	}
};
