import type { LayoutServerLoad } from './$types';

// Auth is now handled client-side via localStorage.
// The server cannot know auth state until hydration completes.
export const load: LayoutServerLoad = async () => {
	return {
		// These are now only used for SSR fallback - actual state comes from client auth store
		loggedIn: false,
		username: null
	};
};
