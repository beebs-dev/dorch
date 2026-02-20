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

		let draft: WadDraft | null = null;
		if (draftId) {
			// Load specific draft
			draft = await wadinfo.getDraft(draftId);
		} else {
			// Resume an existing unpublished draft if one exists (don't create a new one)
			const draftsResp = await wadinfo.listDrafts();
			draft = draftsResp.items.find((d) => d.status === 'draft') ?? null;
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
