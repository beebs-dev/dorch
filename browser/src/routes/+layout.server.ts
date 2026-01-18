import type { LayoutServerLoad } from './$types';

const REFRESH_TOKEN_COOKIE = 'dorch_refresh_token';
const LOGGED_IN_COOKIE = 'dorch_logged_in';

export const load: LayoutServerLoad = async ({ cookies }) => {
	const refreshToken = cookies.get(REFRESH_TOKEN_COOKIE);
	const loggedInCookie = cookies.get(LOGGED_IN_COOKIE);
	return {
		loggedIn:
			loggedInCookie === '1' || (typeof refreshToken === 'string' && refreshToken.length > 0)
	};
};
