import type { Handle } from '@sveltejs/kit';

import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import { isRateLimited } from '$lib/server/rate_limit';

function isPageResponse(response: Response): boolean {
	const contentType = response.headers.get('content-type') ?? '';
	return contentType.includes('text/html');
}

export const handle: Handle = async ({ event, resolve }) => {
    if (await isRateLimited(event)) {
        return new Response('Rate limit exceeded', {
            status: 429,
            headers: {
                'content-type': 'text/plain; charset=utf-8'
            }
        });
    }

	const response = await resolve(event);
    if (isPageResponse(response)) {
        const forwardedFor = getTrustedXForwardedFor(event.request);
        console.log('page_response', {
            path: event.url.pathname,
            status: response.status,
            xff: forwardedFor
        });
    }
	return response;
};
