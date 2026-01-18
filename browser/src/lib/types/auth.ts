export type JwtLike = {
	access_token: string;
	refresh_token?: string | null;
	token_type?: string | null;
	expires_in?: number | null;
	refresh_expires_in?: number | null;
	id_token?: string | null;
	scope?: string | null;
	session_state?: string | null;
};

export type UserCredentials = {
	id: string;
	username: string;
	first_name: string;
	last_name: string;
	email: string;
	jwt: JwtLike;
};
