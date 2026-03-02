import type { PageServerLoad } from './$types';
import { createDorchMasterPublicClient } from '$lib/server/dorchmaster';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import type { GameSummary } from '$lib/types/games';
import type { WadMeta } from '$lib/types/wadinfo';

type ManagedServerRow = {
    game: GameSummary;
    thumbnailUrl?: string;
    iwadName: string;
    pwadName?: string | null;
};

function wadDisplayName(meta: WadMeta): string {
    return meta.title ?? meta.filename ?? meta.filenames?.[0] ?? meta.id;
}

export const load: PageServerLoad = async ({ cookies, fetch, request, setHeaders }) => {
    const accessToken = cookies.get('dorch_access_token');
    if (!accessToken) {
        return {
            rows: [] as ManagedServerRow[],
            notAuthenticated: true,
            errorMessage: null as string | null,
            fetchedAt: Date.now()
        };
    }

    const forwardedFor = getTrustedXForwardedFor(request);
    const dorch = createDorchMasterPublicClient(fetch, { forwardedFor, bearerToken: accessToken });
    const wadinfo = createWadinfoClient(fetch, { forwardedFor, bearerToken: accessToken });

    let games: GameSummary[] = [];
    let errorMessage: string | null = null;
    try {
        const resp = await dorch.listMyGames();
        games = resp?.games ?? [];
    } catch (e) {
        const status = (e as { status?: number })?.status;
        if (status === 401) {
            return {
                rows: [] as ManagedServerRow[],
                notAuthenticated: true,
                errorMessage: null as string | null,
                fetchedAt: Date.now()
            };
        }
        errorMessage = e instanceof Error ? e.message : 'Failed to fetch your servers';
        games = [];
    }

    // Enrich wad names (best-effort). dorch-master returns UUIDs for iwad/files.
    const wantedWadIds = new Set<string>();
    for (const game of games) {
        if (game.iwad) wantedWadIds.add(game.iwad);
        for (const file of game.files ?? []) {
            if (file) wantedWadIds.add(file);
        }
    }

    const wadMetaById = wantedWadIds.size
        ? await wadinfo.getWadMetas([...wantedWadIds])
        : new Map<string, WadMeta>();
    const wadNameById = new Map<string, string>();
    for (const [wadId, meta] of wadMetaById.entries()) {
        wadNameById.set(wadId, wadDisplayName(meta));
    }

    const rows: ManagedServerRow[] = games.map((game) => {
        const pwadNames = (game.files ?? [])
            .filter(Boolean)
            .map((fileId) => wadNameById.get(fileId) ?? fileId)
            .filter((s) => s != 'Doom Shareware v1.9');
        const pwadName = pwadNames.length ? pwadNames.join(' | ') : null;

        const currentMap = game.info?.current_map;
        const wadId = game.files?.[game.files.length - 1] ?? game.iwad;
        const thumbnailUrl =
            currentMap && wadId
                ? `/servers/${encodeURIComponent(game.game_id)}/thumb?wad_id=${encodeURIComponent(wadId)}&map=${encodeURIComponent(currentMap)}`
                : undefined;

        return {
            game,
            thumbnailUrl,
            iwadName: wadNameById.get(game.iwad) ?? game.iwad,
            pwadName
        };
    });

    setHeaders({ 'cache-control': 'private, max-age=0, s-maxage=5' });

    return {
        rows,
        notAuthenticated: false,
        errorMessage,
        fetchedAt: Date.now()
    };
};
