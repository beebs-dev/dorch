import type { LayoutServerLoad } from './$types';

const REFRESH_TOKEN_COOKIE = 'dorch_refresh_token';
const LOGGED_IN_COOKIE = 'dorch_logged_in';
const USERNAME_COOKIE = 'dorch_username';

export const load: LayoutServerLoad = async ({ cookies }) => {
	const refreshToken = cookies.get(REFRESH_TOKEN_COOKIE);
	const loggedInCookie = cookies.get(LOGGED_IN_COOKIE);
	const username = cookies.get(USERNAME_COOKIE);
	const loggedIn =
		loggedInCookie === '1' || (typeof refreshToken === 'string' && refreshToken.length > 0);
	return {
		loggedIn,
		username: loggedIn && typeof username === 'string' && username.length > 0 ? username : null
	};
};
