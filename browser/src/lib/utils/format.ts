import type { WadMeta } from '$lib/types/wadinfo';

export function shortSha(sha1?: string | null, length = 8): string {
	if (!sha1) return '';
	return sha1.slice(0, length);
}

function nonEmptyString(v: unknown): string | null {
	if (typeof v !== 'string') return null;
	const trimmed = v.trim();
	return trimmed.length ? trimmed : null;
}

/**
 * Preferred label for showing WADs in lists (browse/search).
 * Order: title → filename → filenames[] → id.
 */
export function wadLabel(meta: Pick<WadMeta, 'id' | 'title' | 'filename' | 'filenames'>): string {
	return (
		nonEmptyString(meta.title) ??
		nonEmptyString(meta.filename) ??
		(() => {
			const files = (meta.filenames ?? [])
				.map((f) => nonEmptyString(f))
				.filter((f): f is string => Boolean(f));
			return files.length ? files.join(', ') : null;
		})() ??
		meta.id
	);
}

export function humanBytes(bytes?: number | null): string {
	if (bytes == null || Number.isNaN(bytes)) return '—';
	const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'] as const;
	let value = bytes;
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit += 1;
	}
	const digits = value >= 10 || unit === 0 ? 0 : 1;
	return `${value.toFixed(digits)} ${units[unit]}`;
}

export function clampInt(value: number, min: number, max: number): number {
	return Math.max(min, Math.min(max, Math.trunc(value)));
}

export function getStringParam(url: URL, key: string): string | null {
	const v = url.searchParams.get(key);
	if (v == null) return null;
	const trimmed = v.trim();
	return trimmed.length ? trimmed : null;
}

export function getIntParam(url: URL, key: string): number | null {
	const raw = url.searchParams.get(key);
	if (!raw) return null;
	const parsed = Number.parseInt(raw, 10);
	return Number.isFinite(parsed) ? parsed : null;
}

export function withParams(url: URL, next: Record<string, string | number | null | undefined>): string {
	const copy = new URL(url);
	for (const [k, v] of Object.entries(next)) {
		if (v == null || v === '') copy.searchParams.delete(k);
		else copy.searchParams.set(k, String(v));
	}
	// Normalize trailing '?' away
	return copy.pathname + (copy.searchParams.toString() ? `?${copy.searchParams.toString()}` : '');
}
