import { error, redirect } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

function statusFromUnknown(e: unknown): number | null {
	if (!e || typeof e !== 'object') return null;
	const status = (e as Record<string, unknown>).status;
	return typeof status === 'number' ? status : null;
}

export const load: PageServerLoad = async ({ cookies, fetch, params, request }) => {
	const accessToken = cookies.get('dorch_access_token');

	if (!accessToken) {
		return {
			wadId: params.wadId,
			wad: null,
			notAuthenticated: true
		};
	}

	const forwardedFor = getTrustedXForwardedFor(request);
	const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });

	try {
		const wad = await wadinfo.getWad(params.wadId);
		return {
			wadId: params.wadId,
			wad,
			notAuthenticated: false
		};
	} catch (e) {
		const status = statusFromUnknown(e);
		if (status === 401) {
			return {
				wadId: params.wadId,
				wad: null,
				notAuthenticated: true
			};
		}
		if (status === 404) throw error(404, 'WAD not found');
		throw error(status ?? 500, 'Failed to fetch WAD');
	}
};

export const actions: Actions = {
	default: async ({ cookies, fetch, params, request }) => {
		const accessToken = cookies.get('dorch_access_token');
		if (!accessToken) {
			throw error(401, 'Not authenticated');
		}

		const forwardedFor = getTrustedXForwardedFor(request);
		const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });

		const formData = await request.formData();
		const title = formData.get('title');

		try {
			await wadinfo.updateWad(params.wadId, {
				title: typeof title === 'string' && title.trim() ? title.trim() : null
			});
			throw redirect(303, `/wad/${params.wadId}`);
		} catch (e) {
			if ((e as { status?: number }).status === 303) throw e; // re-throw redirect
			const status = statusFromUnknown(e);
			if (status === 401) throw error(401, 'Not authenticated');
			if (status === 403) throw error(403, 'Not authorized to edit this WAD');
			if (status === 404) throw error(404, 'WAD not found');
			console.error('Failed to update WAD:', e);
			throw error(500, 'Failed to update WAD');
		}
	}
};
