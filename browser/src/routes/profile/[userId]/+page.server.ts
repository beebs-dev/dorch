import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

function statusFromUnknown(e: unknown): number | null {
    if (!e || typeof e !== 'object') return null;
    const status = (e as Record<string, unknown>).status;
    return typeof status === 'number' ? status : null;
}

export const load: PageServerLoad = async ({ fetch, params, request, setHeaders }) => {
    const userId = params.userId?.trim();
    if (!userId) {
        throw error(400, 'Missing user ID');
    }

    const forwardedFor = getTrustedXForwardedFor(request);
    const wadinfo = createWadinfoClient(fetch, { forwardedFor });

    try {
        const profile = await wadinfo.getUserProfile(userId);
        setHeaders({ 'cache-control': 'public, max-age=30, s-maxage=30' });
        return { profile };
    } catch (e) {
        const status = statusFromUnknown(e);
        if (status === 404) {
            throw error(404, 'Profile not found');
        }
        throw error(status ?? 502, 'Failed to load profile');
    }
};
