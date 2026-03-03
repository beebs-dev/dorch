<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
	import { browser } from '$app/environment';
	import { showToast } from '$lib/stores/toast';
	import { subscribe as authSubscribe, getAccessToken, type AuthState } from '$lib/stores/auth';
	import type { UserProfileFull } from '$lib/types/wadinfo';
	import {
		LS_PLAYER_COLOR_KEY,
		NAMED_PLAYER_COLORS,
		NAMED_PLAYER_COLOR_INDEX_SET,
		parsePlayerColorIndex,
		toPlayerColorHex
	} from '$lib/utils/playerColor';

	let { open, onClose }: { open: boolean; onClose: () => void } = $props();

	const API_BASE_URL = 'https://api.gib.gg';
	const DEBOUNCE_MS = 300;

	const NAME_MAX_LEN = 20;
	const LS_NAME_KEY = 'dorch.settings.name';
	const LS_PLAYER_COLOR_KEY_FALLBACK = LS_PLAYER_COLOR_KEY;
	const LS_CONFIG_KEY = 'dorch.settings.config';
	const LS_DOOM2_META_KEY = 'dorch.settings.doom2_override';
	const IDB_DB_NAME = 'dorch.settings';
	const IDB_STORE_FILES = 'files';
	const IDB_DOOM2_KEY = 'doom2_override';

	type Doom2OverrideMeta =
		| { present: false }
		| { present: true; name: string; size: number; type: string; lastModified: number };

	let name = $state(''); // persisted
	let playerColor = $state(0); // persisted palette index 0..255
	let playerColorInput = $state('0');
	let config = $state(''); // persisted
	let doom2Meta: Doom2OverrideMeta = $state({ present: false }); // persisted (metadata only)

	let nameError = $state<string | null>(null);
	let playerColorError = $state<string | null>(null);
	let configError = $state<string | null>(null);
	let doom2Error = $state<string | null>(null);

	// Auth state for syncing display name
	let authState = $state<AuthState>({
		isAuthenticated: false,
		userId: null,
		username: null,
		accessToken: null
	});
	let userProfile = $state<UserProfileFull | null>(null);
	let nameDebounceTimer: ReturnType<typeof setTimeout> | null = null;
	let colorDebounceTimer: ReturnType<typeof setTimeout> | null = null;
	let updatingName = $state(false);
	let updatingColor = $state(false);

	let modalEl: HTMLDivElement | null = $state(null);
	let nameEl: HTMLInputElement | null = $state(null);
	let lastActiveEl: HTMLElement | null = null;

	function getFocusable(container: HTMLElement): HTMLElement[] {
		const selector =
			'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';
		return Array.from(container.querySelectorAll<HTMLElement>(selector)).filter((el) => {
			const style = window.getComputedStyle(el);
			return style.visibility !== 'hidden' && style.display !== 'none';
		});
	}

	async function focusFirst() {
		await tick();
		nameEl?.focus();
	}

	function close() {
		onClose();
	}

	function randomIdent(): string {
		const adjectives = [
			'quick',
			'bright',
			'silent',
			'fierce',
			'brave',
			'clever',
			'lucky',
			'wild',
			'calm',
			'proud'
		];
		const nouns = [
			'tiger',
			'eagle',
			'lion',
			'wolf',
			'panther',
			'hawk',
			'fox',
			'bear',
			'dragon',
			'falcon'
		];

		const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
		const noun = nouns[Math.floor(Math.random() * nouns.length)];
		const number = Math.floor(Math.random() * 1000);
		return `${adj}-${noun}-${number}`;
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
			return;
		}

		if (e.key !== 'Tab') return;
		if (!modalEl) return;

		const focusable = getFocusable(modalEl);
		if (focusable.length === 0) return;

		const first = focusable[0];
		const last = focusable[focusable.length - 1];
		const active = document.activeElement as HTMLElement | null;

		if (e.shiftKey) {
			if (!active || active === first) {
				e.preventDefault();
				last.focus();
			}
			return;
		}

		if (!active || active === last) {
			e.preventDefault();
			first.focus();
		}
	}

	function safeJsonParse<T>(value: string | null): T | null {
		if (!value) return null;
		try {
			return JSON.parse(value) as T;
		} catch {
			return null;
		}
	}

	function loadFromLocalStorage() {
		if (!browser) return;
		const storedName = window.localStorage.getItem(LS_NAME_KEY);
		if (storedName && storedName.trim().length > 0) {
			name = storedName;
		} else {
			name = randomIdent();
			window.localStorage.setItem(LS_NAME_KEY, name);
		}
		const localPlayerColor = parsePlayerColorIndex(
			window.localStorage.getItem(LS_PLAYER_COLOR_KEY_FALLBACK)
		);
		playerColor = localPlayerColor ?? 0;
		playerColorInput = String(playerColor);
		config = window.localStorage.getItem(LS_CONFIG_KEY) ?? '';
		doom2Meta = safeJsonParse<Doom2OverrideMeta>(
			window.localStorage.getItem(LS_DOOM2_META_KEY)
		) ?? {
			present: false
		};
	}

	function persistPlayerColor(next: number) {
		playerColorError = null;
		const parsed = parsePlayerColorIndex(next);
		if (parsed === null) {
			playerColorError = 'Player color must be a number between 0 and 255.';
			return;
		}

		playerColor = parsed;
		playerColorInput = String(parsed);

		if (!browser) return;
		window.localStorage.setItem(LS_PLAYER_COLOR_KEY_FALLBACK, String(parsed));

		if (authState.isAuthenticated && userProfile) {
			if (colorDebounceTimer) {
				clearTimeout(colorDebounceTimer);
			}
			colorDebounceTimer = setTimeout(() => {
				updatePlayerColor(parsed);
			}, DEBOUNCE_MS);
		}
	}

	function onPlayerColorInput(nextRaw: string) {
		playerColorInput = nextRaw;
		const parsed = parsePlayerColorIndex(nextRaw);
		if (parsed === null) {
			playerColorError = 'Player color must be a number between 0 and 255.';
			return;
		}
		persistPlayerColor(parsed);
	}

	function selectedPlayerColorOptionValue(): string {
		if (NAMED_PLAYER_COLOR_INDEX_SET.has(playerColor)) {
			return String(playerColor);
		}
		return '__custom__';
	}

	function onPlayerColorSelect(nextRaw: string) {
		if (nextRaw === '__custom__') {
			return;
		}
		const parsed = parsePlayerColorIndex(nextRaw);
		if (parsed === null) return;
		persistPlayerColor(parsed);
	}

	function persistName(next: string) {
		nameError = null;
		if (next.length > NAME_MAX_LEN) {
			nameError = `Name cannot exceed ${NAME_MAX_LEN} characters.`;
			next = next.slice(0, NAME_MAX_LEN);
		}
		name = next;
		if (!browser) return;
		window.localStorage.setItem(LS_NAME_KEY, name);

		// If logged in, debounce update to user profile
		if (authState.isAuthenticated && userProfile) {
			if (nameDebounceTimer) {
				clearTimeout(nameDebounceTimer);
			}
			nameDebounceTimer = setTimeout(() => {
				updateDisplayName(next);
			}, DEBOUNCE_MS);
		}
	}

	async function updateDisplayName(displayName: string) {
		if (!authState.isAuthenticated || !userProfile) return;
		if (displayName.trim() === userProfile.display_name) return;

		updatingName = true;
		try {
			const token = await getAccessToken();
			if (!token) {
				return;
			}

			const res = await fetch(
				`${API_BASE_URL}/user/profile/${encodeURIComponent(userProfile.id)}`,
				{
					method: 'PUT',
					headers: {
						'content-type': 'application/json',
						accept: 'application/json',
						authorization: `Bearer ${token}`
					},
					body: JSON.stringify({ display_name: displayName.trim() })
				}
			);

			if (!res.ok) {
				let msg = 'Failed to update display name.';
				try {
					const body = await res.json();
					if (typeof body?.error === 'string') msg = body.error;
				} catch {
					// ignore
				}
				showToast(msg);
				return;
			}

			const updated = (await res.json()) as UserProfileFull;
			userProfile = updated;
		} catch (err: any) {
			showToast(err?.message ?? 'Failed to update display name.');
		} finally {
			updatingName = false;
		}
	}

	async function updatePlayerColor(nextColor: number) {
		if (!authState.isAuthenticated || !userProfile) return;
		if ((userProfile.player_color ?? null) === nextColor) return;

		updatingColor = true;
		try {
			const token = await getAccessToken();
			if (!token) {
				return;
			}

			const res = await fetch(
				`${API_BASE_URL}/user/profile/${encodeURIComponent(userProfile.id)}`,
				{
					method: 'PUT',
					headers: {
						'content-type': 'application/json',
						accept: 'application/json',
						authorization: `Bearer ${token}`
					},
					body: JSON.stringify({ player_color: nextColor })
				}
			);

			if (!res.ok) {
				let msg = 'Failed to update player color.';
				try {
					const body = await res.json();
					if (typeof body?.error === 'string') msg = body.error;
				} catch {
					// ignore
				}
				showToast(msg);
				return;
			}

			const updated = (await res.json()) as UserProfileFull;
			userProfile = updated;
		} catch (err: any) {
			showToast(err?.message ?? 'Failed to update player color.');
		} finally {
			updatingColor = false;
		}
	}

	async function loadUserProfile() {
		if (!authState.isAuthenticated || !authState.userId) return;

		try {
			const token = await getAccessToken();
			if (!token) return;

			const res = await fetch(
				`${API_BASE_URL}/user/profile/${encodeURIComponent(authState.userId)}`,
				{
					method: 'GET',
					headers: {
						accept: 'application/json',
						authorization: `Bearer ${token}`
					}
				}
			);

			if (!res.ok) return;

			const profile = (await res.json()) as UserProfileFull;
			userProfile = profile;

			// Sync name from profile display_name
			if (profile.display_name) {
				name = profile.display_name;
				window.localStorage.setItem(LS_NAME_KEY, name);
			}
			const profilePlayerColor = parsePlayerColorIndex(profile.player_color);
			if (profilePlayerColor !== null) {
				playerColor = profilePlayerColor;
				playerColorInput = String(profilePlayerColor);
				window.localStorage.setItem(LS_PLAYER_COLOR_KEY_FALLBACK, String(profilePlayerColor));
			}
		} catch {
			// Ignore - user can still use local name
		}
	}

	function persistConfig(next: string) {
		configError = null;
		config = next;
		if (!browser) return;
		window.localStorage.setItem(LS_CONFIG_KEY, config);
	}

	function persistDoom2Meta(next: Doom2OverrideMeta) {
		doom2Meta = next;
		if (!browser) return;
		window.localStorage.setItem(LS_DOOM2_META_KEY, JSON.stringify(doom2Meta));
	}

	function isProbablyText(bytes: Uint8Array): boolean {
		// Reject if it contains NUL bytes (very common for binaries).
		for (const b of bytes) {
			if (b === 0) return false;
		}

		// Heuristic: if too many control chars (except \n \r \t), consider it non-text.
		let control = 0;
		for (const b of bytes) {
			if (b < 9) control++;
			else if (b > 13 && b < 32) control++;
		}
		return control / Math.max(1, bytes.length) < 0.05;
	}

	async function readTextFileOrError(
		file: File
	): Promise<{ ok: true; text: string } | { ok: false; error: string }> {
		try {
			// Fast check: if browser tells us it's text/* we accept.
			if (file.type && file.type.startsWith('text/')) {
				return { ok: true, text: await file.text() };
			}

			// Otherwise sample first 4KB and apply a binary heuristic.
			const sampleBuf = await file.slice(0, 4096).arrayBuffer();
			const sample = new Uint8Array(sampleBuf);
			if (!isProbablyText(sample)) {
				return { ok: false, error: 'That file does not appear to be plain text.' };
			}

			return { ok: true, text: await file.text() };
		} catch {
			return { ok: false, error: 'Failed to read the uploaded file.' };
		}
	}

	function openDb(): Promise<IDBDatabase> {
		return new Promise((resolve, reject) => {
			const req = indexedDB.open(IDB_DB_NAME, 1);
			req.onupgradeneeded = () => {
				const db = req.result;
				if (!db.objectStoreNames.contains(IDB_STORE_FILES)) {
					db.createObjectStore(IDB_STORE_FILES);
				}
			};
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
		});
	}

	async function idbPut(key: string, value: unknown) {
		const db = await openDb();
		try {
			await new Promise<void>((resolve, reject) => {
				const tx = db.transaction(IDB_STORE_FILES, 'readwrite');
				tx.oncomplete = () => resolve();
				tx.onerror = () => reject(tx.error);
				tx.objectStore(IDB_STORE_FILES).put(value, key);
			});
		} finally {
			db.close();
		}
	}

	async function idbDelete(key: string) {
		const db = await openDb();
		try {
			await new Promise<void>((resolve, reject) => {
				const tx = db.transaction(IDB_STORE_FILES, 'readwrite');
				tx.oncomplete = () => resolve();
				tx.onerror = () => reject(tx.error);
				tx.objectStore(IDB_STORE_FILES).delete(key);
			});
		} finally {
			db.close();
		}
	}

	async function onUploadConfig(e: Event) {
		configError = null;
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;

		const res = await readTextFileOrError(file);
		if (!res.ok) {
			configError = res.error;
			return;
		}
		persistConfig(res.text);
	}

	async function onUploadDoom2(e: Event) {
		doom2Error = null;
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;

		try {
			// Store the raw bytes in IndexedDB, and persist metadata in localStorage.
			const buf = await file.arrayBuffer();
			await idbPut(IDB_DOOM2_KEY, {
				name: file.name,
				size: file.size,
				type: file.type,
				lastModified: file.lastModified,
				data: buf
			});
			persistDoom2Meta({
				present: true,
				name: file.name,
				size: file.size,
				type: file.type,
				lastModified: file.lastModified
			});
			showToast('doom2.wad override saved in browser storage.');
		} catch {
			doom2Error = 'Failed to store the selected file.';
		}
	}

	async function resetDoom2() {
		doom2Error = null;
		try {
			await idbDelete(IDB_DOOM2_KEY);
		} catch {
			// ignore
		}
		persistDoom2Meta({ present: false });
	}

	// Subscribe to auth state changes (runs once on mount)
	$effect(() => {
		if (!browser) return;
		const unsub = authSubscribe((state) => {
			authState = state;
			if (!state.isAuthenticated) {
				userProfile = null;
			}
		});
		return unsub;
	});

	$effect(() => {
		if (!browser) return;
		if (!open) return;

		return untrack(() => {
			loadFromLocalStorage();

			lastActiveEl = document.activeElement instanceof HTMLElement ? document.activeElement : null;
			focusFirst();

			const onDocKeydown = (e: KeyboardEvent) => onKeydown(e);
			document.addEventListener('keydown', onDocKeydown);

			const prevOverflow = document.documentElement.style.overflow;
			document.documentElement.style.overflow = 'hidden';

			if (authState.isAuthenticated) {
				void loadUserProfile();
			}

			return () => {
				document.removeEventListener('keydown', onDocKeydown);
				document.documentElement.style.overflow = prevOverflow;
			};
		});
	});

	onDestroy(() => {
		if (!browser) return;
		if (nameDebounceTimer) {
			clearTimeout(nameDebounceTimer);
		}
		if (colorDebounceTimer) {
			clearTimeout(colorDebounceTimer);
		}
		queueMicrotask(() => lastActiveEl?.focus());
	});
</script>

{#if open}
	<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
		<button
			type="button"
			class="absolute inset-0 bg-zinc-950/80"
			onclick={close}
			aria-label="Close settings dialog"
		></button>

		<div
			bind:this={modalEl}
			class="relative w-full max-w-2xl overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
			role="dialog"
			aria-modal="true"
			aria-label="Settings"
			tabindex="-1"
		>
			<div class="flex items-center justify-between border-b border-zinc-800/80 px-5 py-4">
				<div>
					<h2 class="text-base font-semibold tracking-wide text-zinc-100">Game Settings</h2>
				</div>
				<button
					type="button"
					class="cursor-pointer rounded-md p-2 text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-100 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
					onclick={close}
					aria-label="Close"
				>
					<span aria-hidden="true">✕</span>
				</button>
			</div>

			<div class="px-5 py-4">
				<div class="grid gap-6">
					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Display Name</h3>
						<p class="mt-1 text-xs text-zinc-500">
							Your in-game display name. Max {NAME_MAX_LEN} characters.
						</p>
						<input
							bind:this={nameEl}
							value={name}
							oninput={(e) => persistName((e.currentTarget as HTMLInputElement).value)}
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="Player name"
							autocomplete="nickname"
						/>
						{#if nameError}
							<p class="mt-2 text-xs text-red-300">{nameError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Player Color</h3>
						<p class="mt-1 text-xs text-zinc-500">
							Doom palette index (0-255) used for your player color.
						</p>
						<div class="mt-2 flex items-stretch gap-2">
							<select
								value={selectedPlayerColorOptionValue()}
								onchange={(e) => onPlayerColorSelect((e.currentTarget as HTMLSelectElement).value)}
								class="min-w-0 flex-1 rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset focus:ring-2 focus:ring-red-700 focus:outline-none"
							>
								{#if !NAMED_PLAYER_COLOR_INDEX_SET.has(playerColor)}
									<option value="__custom__">Custom Color</option>
								{/if}
								{#each NAMED_PLAYER_COLORS as item}
									<option value={String(item.paletteIndex)}
										>{item.label} (#{item.paletteIndex})</option
									>
								{/each}
							</select>
							<input
								type="number"
								min="0"
								max="255"
								step="1"
								inputmode="numeric"
								value={playerColorInput}
								oninput={(e) => onPlayerColorInput((e.currentTarget as HTMLInputElement).value)}
								class="w-28 rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset focus:ring-2 focus:ring-red-700 focus:outline-none"
								aria-label="Player color index"
							/>
						</div>
						<div class="mt-2 flex items-center gap-2 text-xs text-zinc-500">
							<span
								class="inline-block h-3 w-3 rounded-sm ring-1 ring-zinc-700"
								style={`background-color: ${toPlayerColorHex(playerColor)}`}
							></span>
							{#if updatingColor}
								<span class="text-zinc-400">Saving…</span>
							{/if}
						</div>
						{#if playerColorError}
							<p class="mt-2 text-xs text-red-300">{playerColorError}</p>
						{/if}
					</section>

					<section>
						<div class="flex items-baseline justify-between gap-4">
							<div>
								<h3 class="text-xs font-semibold tracking-wide text-zinc-300">
									Config (autoexec.cfg)
								</h3>
								<p class="mt-1 text-xs text-zinc-500">Paste text or upload a text file.</p>
							</div>
							<label
								class="inline-flex cursor-pointer items-center gap-2 rounded-md bg-zinc-900 px-3 py-2 text-xs font-semibold text-zinc-100 ring-1 ring-zinc-800 focus-within:ring-2 focus-within:ring-zinc-500 hover:bg-zinc-800"
							>
								<input type="file" class="hidden" oninput={onUploadConfig} />
								Upload
							</label>
						</div>

						<textarea
							rows={8}
							bind:value={config}
							oninput={(e) => persistConfig((e.currentTarget as HTMLTextAreaElement).value)}
							class="mt-2 w-full resize-y rounded-lg bg-zinc-950 px-3 py-2 text-xs font-[var(--dorch-mono)] text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="# config goes here"
						></textarea>
						{#if configError}
							<p class="mt-2 text-xs text-red-300">{configError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">doom2.wad Override</h3>
						<p class="mt-1 text-xs text-zinc-500">
							Select a local file to replace freedoom2.wad. If you don't choose one, the default
							freedoom2.wad will be used. Your selected file stays in your browser and is never
							uploaded. Use this option if you want to play using assets from the original retail
							game.
						</p>
						<div class="mt-2 flex flex-wrap items-center gap-3">
							<div
								class="inline-flex items-center gap-2 rounded-md bg-zinc-950 px-3 py-2 ring-1 ring-zinc-800"
							>
								<span
									class={`h-2.5 w-2.5 rounded-full ${doom2Meta.present ? 'bg-emerald-500' : 'bg-red-500'}`}
									aria-hidden="true"
								></span>
								<span class="text-xs font-semibold text-zinc-200">
									{doom2Meta.present ? 'OK' : 'None'}
								</span>
							</div>

							<label
								class="inline-flex cursor-pointer items-center gap-2 rounded-md bg-zinc-900 px-3 py-2 text-xs font-semibold text-zinc-100 ring-1 ring-zinc-800 focus-within:ring-2 focus-within:ring-zinc-500 hover:bg-zinc-800"
							>
								<input type="file" class="hidden" accept=".wad" oninput={onUploadDoom2} />
								Select file
							</label>

							<button
								type="button"
								class="cursor-pointer rounded-md bg-transparent px-3 py-2 text-xs font-semibold text-red-300 ring-1 ring-red-900/60 hover:bg-red-950/40 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
								onclick={resetDoom2}
								disabled={!doom2Meta.present}
							>
								Reset
							</button>
						</div>

						{#if doom2Meta.present}
							<p class="mt-2 text-xs text-zinc-400">
								Selected: <span class="font-semibold text-zinc-200">{doom2Meta.name}</span>
								<span class="ml-2 text-zinc-500"
									>({Math.round(doom2Meta.size / 1024 / 1024)} MB)</span
								>
							</p>
						{/if}
						{#if doom2Error}
							<p class="mt-2 text-xs text-red-300">{doom2Error}</p>
						{/if}
					</section>
				</div>
			</div>
		</div>
	</div>
{/if}
