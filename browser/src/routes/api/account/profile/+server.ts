import { json, type RequestHandler } from '@sveltejs/kit';
import { createWadinfoClient } from '$lib/server/wadinfo';
import { getTrustedXForwardedFor } from '$lib/server/forwarded';
import { getAuthenticatedSession } from '$lib/server/session';
import type { PutUserProfileRequest } from '$lib/types/wadinfo';

function asMaybeString(value: unknown): string | null | undefined {
	if (value === undefined) return undefined;
	if (value === null) return null;
	if (typeof value !== 'string') return undefined;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

function normalizePutPayload(raw: unknown): PutUserProfileRequest | null {
	if (!raw || typeof raw !== 'object') return null;
	const obj = raw as Record<string, unknown>;
	const avatarUrl = asMaybeString(obj.avatar_url);
	if (obj.avatar_url !== undefined && avatarUrl === undefined) return null;

	if (obj.username !== undefined && typeof obj.username !== 'string') return null;
	const username = typeof obj.username === 'string' ? obj.username.trim() : undefined;
	if (username !== undefined && username.length === 0) return null;

	if (obj.privacy_hide_activity !== undefined && typeof obj.privacy_hide_activity !== 'boolean') {
		return null;
	}

	return {
		...(avatarUrl !== undefined ? { avatar_url: avatarUrl } : {}),
		...(username !== undefined ? { username } : {}),
		...(typeof obj.privacy_hide_activity === 'boolean'
			? { privacy_hide_activity: obj.privacy_hide_activity }
			: {})
	};
}

export const GET: RequestHandler = async (event) => {
	const session = await getAuthenticatedSession(event);
	if (!session) {
		return json({ error: 'Not authenticated' }, { status: 401 });
	}
	const forwardedFor = getTrustedXForwardedFor(event.request);
	const wadinfo = createWadinfoClient(event.fetch, {
		forwardedFor,
		bearerToken: session.accessToken
	});

	try {
		const profile = await wadinfo.getUserProfile(session.userId);
		return json(profile, {
			headers: {
				'cache-control': 'private, max-age=0'
			}
		});
	} catch (err: any) {
		const status = typeof err?.status === 'number' ? err.status : 500;
		const msg = status === 404 ? 'Profile not found' : 'Failed to load profile';
		return json({ error: msg }, { status });
	}
};

export const PUT: RequestHandler = async (event) => {
	const session = await getAuthenticatedSession(event);
	if (!session) {
		return json({ error: 'Not authenticated' }, { status: 401 });
	}

	let payloadRaw: unknown;
	try {
		payloadRaw = await event.request.json();
	} catch {
		return json({ error: 'Invalid JSON body' }, { status: 400 });
	}

	const payload = normalizePutPayload(payloadRaw);
	if (!payload) {
		return json({ error: 'Invalid payload' }, { status: 400 });
	}

	const forwardedFor = getTrustedXForwardedFor(event.request);
	const wadinfo = createWadinfoClient(event.fetch, {
		forwardedFor,
		bearerToken: session.accessToken
	});

	try {
		const updated = await wadinfo.putUserProfile(session.userId, payload);
		return json(updated, {
			headers: {
				'cache-control': 'private, max-age=0'
			}
		});
	} catch (err: any) {
		const status = typeof err?.status === 'number' ? err.status : 500;
		const msg = status === 404 ? 'Profile not found' : 'Failed to update profile';
		return json({ error: msg }, { status });
	}
};
