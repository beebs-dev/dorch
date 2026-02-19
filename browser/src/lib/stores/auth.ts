import { browser } from '$app/environment';
import type { JwtLike, UserCredentials } from '$lib/types/auth';

const API_BASE_URL = 'https://api.gib.gg';
const ACCESS_TOKEN_COMFORT_WINDOW_MS = 60_000; // Refresh 1 minute before expiry

const STORAGE_KEY_ACCESS_TOKEN = 'dorch_access_token';
const STORAGE_KEY_ACCESS_TOKEN_EXP = 'dorch_access_token_exp';
const STORAGE_KEY_REFRESH_TOKEN = 'dorch_refresh_token';
const STORAGE_KEY_REFRESH_TOKEN_EXP = 'dorch_refresh_token_exp';
const STORAGE_KEY_USER_ID = 'dorch_user_id';
const STORAGE_KEY_USERNAME = 'dorch_username';

const COOKIE_ACCESS_TOKEN = 'dorch_access_token';
const COOKIE_USER_ID = 'dorch_user_id';

function setCookie(name: string, value: string, maxAgeSeconds: number): void {
	if (!browser) return;
	document.cookie = `${name}=${encodeURIComponent(value)}; path=/; max-age=${maxAgeSeconds}; SameSite=Lax`;
}

function clearCookie(name: string): void {
	if (!browser) return;
	document.cookie = `${name}=; path=/; max-age=0; SameSite=Lax`;
}

export type AuthState = {
	isAuthenticated: boolean;
	userId: string | null;
	username: string | null;
	accessToken: string | null;
};

type Listener = (state: AuthState) => void;

let currentState: AuthState = {
	isAuthenticated: false,
	userId: null,
	username: null,
	accessToken: null
};

const listeners = new Set<Listener>();

function notifyListeners() {
	for (const listener of listeners) {
		listener(currentState);
	}
}

function getStorage(): Storage | null {
	if (!browser) return null;
	return localStorage;
}

function loadFromStorage(): void {
	const storage = getStorage();
	if (!storage) return;

	const accessToken = storage.getItem(STORAGE_KEY_ACCESS_TOKEN);
	const accessTokenExp = storage.getItem(STORAGE_KEY_ACCESS_TOKEN_EXP);
	const refreshToken = storage.getItem(STORAGE_KEY_REFRESH_TOKEN);
	const userId = storage.getItem(STORAGE_KEY_USER_ID);
	const username = storage.getItem(STORAGE_KEY_USERNAME);

	// Check if access token is still valid
	if (accessToken && accessTokenExp) {
		const expMs = parseInt(accessTokenExp, 10);
		if (!Number.isNaN(expMs) && Date.now() < expMs) {
			currentState = {
				isAuthenticated: true,
				userId,
				username,
				accessToken
			};
			notifyListeners();
			return;
		}
	}

	// Access token expired or missing - try to use refresh token
	if (refreshToken) {
		// Attempt refresh asynchronously
		refreshAccessToken().catch(() => {
			clearAuth();
		});
	}
}

function saveToStorage(creds: UserCredentials, persist: boolean): void {
	const storage = getStorage();
	if (!storage) return;

	const { jwt, id, username } = creds;
	const accessToken = jwt.access_token;
	const refreshToken = jwt.refresh_token;
	const expiresIn = jwt.expires_in ?? 300; // Default 5 minutes
	const refreshExpiresIn = jwt.refresh_expires_in ?? (30 * 86400); // Default 30 days

	const accessTokenExpMs = Date.now() + expiresIn * 1000;
	const refreshTokenExpMs = Date.now() + refreshExpiresIn * 1000;

	storage.setItem(STORAGE_KEY_ACCESS_TOKEN, accessToken);
	storage.setItem(STORAGE_KEY_ACCESS_TOKEN_EXP, String(accessTokenExpMs));
	storage.setItem(STORAGE_KEY_USER_ID, id);
	storage.setItem(STORAGE_KEY_USERNAME, username);

	// Also set cookies so the server can read them for SSR
	setCookie(COOKIE_ACCESS_TOKEN, accessToken, expiresIn);
	setCookie(COOKIE_USER_ID, id, expiresIn);

	if (refreshToken && persist) {
		storage.setItem(STORAGE_KEY_REFRESH_TOKEN, refreshToken);
		storage.setItem(STORAGE_KEY_REFRESH_TOKEN_EXP, String(refreshTokenExpMs));
	}

	currentState = {
		isAuthenticated: true,
		userId: id,
		username,
		accessToken
	};
	notifyListeners();
}

function clearStorage(): void {
	const storage = getStorage();
	if (!storage) return;

	storage.removeItem(STORAGE_KEY_ACCESS_TOKEN);
	storage.removeItem(STORAGE_KEY_ACCESS_TOKEN_EXP);
	storage.removeItem(STORAGE_KEY_REFRESH_TOKEN);
	storage.removeItem(STORAGE_KEY_REFRESH_TOKEN_EXP);
	storage.removeItem(STORAGE_KEY_USER_ID);
	storage.removeItem(STORAGE_KEY_USERNAME);

	// Also clear cookies
	clearCookie(COOKIE_ACCESS_TOKEN);
	clearCookie(COOKIE_USER_ID);
}

export function clearAuth(): void {
	clearStorage();
	currentState = {
		isAuthenticated: false,
		userId: null,
		username: null,
		accessToken: null
	};
	notifyListeners();
}

export async function login(
	username: string,
	password: string,
	rememberMe: boolean = true
): Promise<UserCredentials> {
	const res = await fetch(`${API_BASE_URL}/user/login`, {
		method: 'POST',
		headers: {
			'content-type': 'application/json',
			accept: 'application/json'
		},
		body: JSON.stringify({ username, password })
	});

	if (!res.ok) {
		let message = 'Login failed';
		try {
			const body = await res.json();
			if (typeof body?.error === 'string') message = body.error;
		} catch {
			// ignore
		}
		throw new Error(message);
	}

	const creds: UserCredentials = await res.json();
	saveToStorage(creds, rememberMe);
	return creds;
}

export function logout(): void {
	clearAuth();
}

export async function refreshAccessToken(): Promise<string | null> {
	const storage = getStorage();
	if (!storage) return null;

	const refreshToken = storage.getItem(STORAGE_KEY_REFRESH_TOKEN);
	if (!refreshToken) return null;

	const refreshTokenExp = storage.getItem(STORAGE_KEY_REFRESH_TOKEN_EXP);
	if (refreshTokenExp) {
		const expMs = parseInt(refreshTokenExp, 10);
		if (!Number.isNaN(expMs) && Date.now() >= expMs) {
			// Refresh token expired
			clearAuth();
			return null;
		}
	}

	try {
		const res = await fetch(`${API_BASE_URL}/user/refresh`, {
			method: 'POST',
			headers: {
				'content-type': 'application/json',
				accept: 'application/json'
			},
			body: JSON.stringify({ refresh_token: refreshToken })
		});

		if (!res.ok) {
			clearAuth();
			return null;
		}

		const creds: UserCredentials = await res.json();
		saveToStorage(creds, true);
		return creds.jwt.access_token;
	} catch {
		clearAuth();
		return null;
	}
}

/**
 * Get a valid access token, refreshing if necessary.
 * Returns null if not authenticated.
 */
export async function getAccessToken(): Promise<string | null> {
	const storage = getStorage();
	if (!storage) return currentState.accessToken;

	const accessToken = storage.getItem(STORAGE_KEY_ACCESS_TOKEN);
	const accessTokenExp = storage.getItem(STORAGE_KEY_ACCESS_TOKEN_EXP);

	if (accessToken && accessTokenExp) {
		const expMs = parseInt(accessTokenExp, 10);
		// Check if token is still comfortable (not about to expire)
		if (!Number.isNaN(expMs) && Date.now() + ACCESS_TOKEN_COMFORT_WINDOW_MS < expMs) {
			return accessToken;
		}
	}

	// Token expired or about to expire - try to refresh
	return refreshAccessToken();
}

/**
 * Get current auth state synchronously.
 */
export function getAuthState(): AuthState {
	return currentState;
}

/**
 * Subscribe to auth state changes.
 */
export function subscribe(listener: Listener): () => void {
	listeners.add(listener);
	// Immediately call with current state
	listener(currentState);
	return () => {
		listeners.delete(listener);
	};
}

/**
 * Initialize auth state from storage.
 * Call this on app startup.
 */
export function initAuth(): void {
	if (browser) {
		loadFromStorage();
	}
}

// Auto-initialize if in browser
if (browser) {
	loadFromStorage();
}
