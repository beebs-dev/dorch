import { writable } from 'svelte/store';

export const toastMessage = writable<string | null>(null);

let toastTimer: ReturnType<typeof setTimeout> | null = null;

export function showToast(message: string, durationMs = 1800) {
	toastMessage.set(message);
	if (toastTimer) clearTimeout(toastTimer);
	toastTimer = setTimeout(() => {
		toastMessage.set(null);
	}, durationMs);
}
