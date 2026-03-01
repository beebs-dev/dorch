import { error, json, type RequestHandler } from '@sveltejs/kit';
import {
    createDorchMasterClient,
    type CreateGameRequest,
    type CreateGameResponse
} from '$lib/server/dorchmaster';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';

function getBearerToken(request: Request, cookieToken: string | undefined): string | null {
    const auth = request.headers.get('authorization') ?? '';
    if (auth.toLowerCase().startsWith('bearer ')) {
        const token = auth.slice(7).trim();
        if (token) return token;
    }
    return cookieToken?.trim() || null;
}

function normalizeCreateGameRequest(input: unknown): CreateGameRequest {
    if (!input || typeof input !== 'object') throw error(400, 'Invalid JSON body');
    const body = input as Record<string, unknown>;

    const name = typeof body.name === 'string' ? body.name.trim() : '';
    if (!name) throw error(400, 'Missing required field: name');

    const iwad = typeof body.iwad === 'string' ? body.iwad.trim() : '';
    if (!iwad) throw error(400, 'Missing required field: iwad');

    const out: CreateGameRequest = {
        name,
        iwad,
        user_ids: []
    };

    if (typeof body.private === 'boolean') out.private = body.private;
    if (typeof body.warp === 'string' && body.warp.trim()) out.warp = body.warp.trim();
    if (typeof body.skill === 'number' && Number.isInteger(body.skill)) out.skill = body.skill;
    if (typeof body.frag_limit === 'number' && Number.isInteger(body.frag_limit) && body.frag_limit >= 0) {
        out.frag_limit = body.frag_limit;
    }
    if (typeof body.time_limit === 'number' && Number.isInteger(body.time_limit) && body.time_limit >= 0) {
        out.time_limit = body.time_limit;
    }
    if (typeof body.motd === 'string' && body.motd.trim()) out.motd = body.motd.trim();
    if (Array.isArray(body.files)) {
        const files = body.files
            .filter((v): v is string => typeof v === 'string')
            .map((v) => v.trim())
            .filter(Boolean);
        if (files.length) out.files = files;
    }

    return out;
}

export const POST: RequestHandler = async ({ fetch, request, cookies }) => {
    const bearerToken = getBearerToken(request, cookies.get('dorch_access_token'));
    if (!bearerToken) throw error(401, 'Authentication required');

    const payload = normalizeCreateGameRequest(await request.json());
    const forwardedFor = getTrustedXForwardedFor(request);
    const dorch = createDorchMasterClient(fetch, { forwardedFor, bearerToken });

    const created = await dorch.createGame(payload);
    return json(created satisfies CreateGameResponse, {
        headers: {
            'cache-control': 'private, max-age=0'
        }
    });
};
