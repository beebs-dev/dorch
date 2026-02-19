import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import type { WadDraft } from '$lib/types/wadinfo';

export const load: PageServerLoad = async ({ cookies, fetch, request, url }) => {
	const accessToken = cookies.get('dorch_access_token');

	if (!accessToken) {
		return {
			draft: null as WadDraft | null,
			notAuthenticated: true
		};
	}

	const forwardedFor = request.headers.get('x-forwarded-for') ?? undefined;
	const draftId = url.searchParams.get('draft');

	try {
		const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });

		let draft: WadDraft;
		if (draftId) {
			// Load specific draft
			draft = await wadinfo.getDraft(draftId);
		} else {
			// Resume existing unpublished or create new
			draft = await wadinfo.resumeOrCreateDraft();
		}

		return {
			draft,
			notAuthenticated: false
		};
	} catch (err) {
		console.error('Failed to load draft:', err);
		const status = (err as { status?: number })?.status;
		if (status === 401) {
			return {
				draft: null as WadDraft | null,
				notAuthenticated: true
			};
		}
		return {
			draft: null as WadDraft | null,
			notAuthenticated: false,
			loadError: 'Failed to load draft'
		};
	}
};
