export function getTrustedXForwardedFor(request: Request): string | undefined {
	const raw = request.headers.get('x-forwarded-for');
	if (!raw) return undefined;
	const value = raw.trim();
	return value.length ? value : undefined;
}
