<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { browser } from '$app/environment';
	import { showToast } from '$lib/stores/toast';
	import { humanBytes, wadLabel } from '$lib/utils/format';
	import type { WadMeta } from '$lib/types/wadinfo';

	type MapOption = { map: string; title?: string | null };
		type Props = {
		open: boolean;
		onClose: () => void;
		wadId: string;
		wadTitle?: string | null;
		maps: MapOption[];
		wadIsIwad?: boolean;
	};

	let { open, onClose, wadId, wadTitle = null, maps, wadIsIwad = false }: Props = $props();

	let modalEl: HTMLDivElement | null = $state(null);
	let nameEl: HTMLInputElement | null = $state(null);
	let lastActiveEl: HTMLElement | null = null;

	let serverName = $state('');
	let iwadUuid = $state('');
	type PwadEntry = { key: string; id: string };
	let pwads = $state<PwadEntry[]>([]);
	let warp = $state('');
	let skill = $state<number>(3);
	let singlePlayer = $state(true);
	let maxPlayers = $state<number>(8);
	let pwadKeyCounter = 0;

	type WadMetaLookupItem =
		| { id: string; ok: true; meta: WadMeta }
		| { id: string; ok: false; status?: number; error: string };

	let wadMetaLoading = $state(false);
	let wadMetaError = $state<string | null>(null);
	let resolvedIwad = $state<WadMetaLookupItem | null>(null);
	let resolvedPwads = $state<Array<WadMetaLookupItem | null>>([]);
	let wadMetaAbort = $state<AbortController | null>(null);
	let wadMetaToken = 0;
	let dragKey = $state<string | null>(null);

	// IWAD search UI
	let iwadSearch = $state('');
	let iwadSearchOpen = $state(false);
	let iwadSearchLoading = $state(false);
	let iwadSearchError = $state<string | null>(null);
	type WadSearchMeta = Pick<WadMeta, 'id' | 'title' | 'filename' | 'filenames'> & {
		file: { type?: string; size?: number | null };
	};
	let iwadSearchItems = $state<Array<{ id: string; meta: WadSearchMeta }>>([]);
	let iwadSearchAbort = $state<AbortController | null>(null);
	let iwadSearchWrap: HTMLDivElement | null = $state(null);
	let iwadSearchTimer = $state<number | null>(null);

	// PWAD search UI (single input at bottom)
	let pwadSearch = $state('');
	let pwadSearchOpen = $state(false);
	let pwadSearchLoading = $state(false);
	let pwadSearchError = $state<string | null>(null);
	let pwadSearchItems = $state<Array<{ id: string; meta: WadSearchMeta }>>([]);
	let pwadSearchAbort = $state<AbortController | null>(null);
	let pwadSearchWrap: HTMLDivElement | null = $state(null);
	let pwadSearchTimer = $state<number | null>(null);

	const DEFAULT_IWAD_UUID_FOR_PWAD = '17bdc0a8-8a81-4b00-90d1-972bf406fa10';

	let nameError = $state<string | null>(null);
	let iwadError = $state<string | null>(null);
	let pwadError = $state<string | null>(null);
	let warpError = $state<string | null>(null);
	let skillError = $state<string | null>(null);
	let maxPlayersError = $state<string | null>(null);

	let didInit = $state(false);

	function close() {
		onClose();
	}

	function makePwadKey(): string {
		pwadKeyCounter += 1;
		return `${Date.now()}-${pwadKeyCounter}`;
	}

	function normalizeUuidList(values: string[]): string[] {
		const out: string[] = [];
		const seen = new Set<string>();
		for (const raw of values) {
			const id = raw.trim();
			if (!id) continue;
			const key = id.toLowerCase();
			if (seen.has(key)) continue;
			seen.add(key);
			out.push(id);
		}
		return out;
	}

	function addPwad(id = '') {
		pwads = [...pwads, { key: makePwadKey(), id }];
		pwadError = null;
	}

	function removePwad(key: string) {
		pwads = pwads.filter((p) => p.key !== key);
		pwadError = null;
	}

	function updatePwad(key: string, id: string) {
		pwads = pwads.map((p) => (p.key === key ? { ...p, id } : p));
		pwadError = null;
	}

	function reorderPwads(fromKey: string, toKey: string) {
		if (fromKey === toKey) return;
		const fromIdx = pwads.findIndex((p) => p.key === fromKey);
		const toIdx = pwads.findIndex((p) => p.key === toKey);
		if (fromIdx < 0 || toIdx < 0) return;
		const next = [...pwads];
		const [moved] = next.splice(fromIdx, 1);
		next.splice(toIdx, 0, moved);
		pwads = next;
	}

	function metaLabel(item: WadMetaLookupItem | null): string {
		if (!item) return '—';
		if (!item.ok) return `${item.id} — ${item.error}`;
		const title = wadLabel(item.meta);
		return title ? `${title}` : item.id;
	}

	function shortId(id: string, max = 36): string {
		const s = id.trim();
		if (s.length <= max) return s;
		return `${s.slice(0, Math.max(0, max - 1))}…`;
	}

	function addPwadIfMissing(id: string) {
		const trimmed = id.trim();
		if (!trimmed) return;
		const key = trimmed.toLowerCase();
		if (pwads.some((p) => p.id.trim().toLowerCase() === key)) return;
		addPwad(trimmed);
	}

	async function searchWads(query: string, which: 'iwad' | 'pwad') {
		const q = query.trim();
		if (which === 'iwad') {
			iwadSearchError = null;
			iwadSearchItems = [];
			if (!q) {
				iwadSearchLoading = false;
				return;
			}
			iwadSearchLoading = true;
			iwadSearchAbort?.abort();
			const ctrl = new AbortController();
			iwadSearchAbort = ctrl;
			try {
				const url = new URL('/api/wad/search', window.location.origin);
				url.searchParams.set('q', q);
				url.searchParams.set('limit', '12');
				const res = await fetch(url, { signal: ctrl.signal });
				if (!res.ok) throw new Error(`search failed: ${res.status}`);
				const payload = (await res.json()) as { items: Array<{ id: string; meta: WadSearchMeta }> };
				iwadSearchItems = payload.items ?? [];
			} catch (e) {
				if ((e as { name?: string }).name === 'AbortError') return;
				iwadSearchError = 'Search failed.';
			} finally {
				iwadSearchLoading = false;
			}
			return;
		}

		pwadSearchError = null;
		pwadSearchItems = [];
		if (!q) {
			pwadSearchLoading = false;
			return;
		}
		pwadSearchLoading = true;
		pwadSearchAbort?.abort();
		const ctrl = new AbortController();
		pwadSearchAbort = ctrl;
		try {
			const url = new URL('/api/wad/search', window.location.origin);
			url.searchParams.set('q', q);
			url.searchParams.set('limit', '12');
			const res = await fetch(url, { signal: ctrl.signal });
			if (!res.ok) throw new Error(`search failed: ${res.status}`);
			const payload = (await res.json()) as { items: Array<{ id: string; meta: WadSearchMeta }> };
			pwadSearchItems = payload.items ?? [];
		} catch (e) {
			if ((e as { name?: string }).name === 'AbortError') return;
			pwadSearchError = 'Search failed.';
		} finally {
			pwadSearchLoading = false;
		}
	}

	function scheduleSearch(which: 'iwad' | 'pwad') {
		if (!browser) return;
		if (which === 'iwad') {
			if (iwadSearchTimer != null) window.clearTimeout(iwadSearchTimer);
			iwadSearchTimer = window.setTimeout(() => {
				if (!open) return;
				if (iwadUuid.trim()) return;
				if (!iwadSearchOpen) return;
				void searchWads(iwadSearch, 'iwad');
			}, 400);
			return;
		}

		if (pwadSearchTimer != null) window.clearTimeout(pwadSearchTimer);
		pwadSearchTimer = window.setTimeout(() => {
			if (!open) return;
			if (!pwadSearchOpen) return;
			void searchWads(pwadSearch, 'pwad');
		}, 400);
	}

	function metaSubLabel(item: WadMetaLookupItem | null): string {
		if (!item) return '';
		if (!item.ok) return item.status ? `HTTP ${item.status}` : '';
		const type = item.meta.file?.type ?? '—';
		const size = humanBytes(item.meta.file?.size ?? null);
		return `${type} • ${size}`;
	}

	async function fetchWadMeta(iwadId: string, pwadIdsRaw: string[]) {
		const token = ++wadMetaToken;
		wadMetaError = null;
		wadMetaLoading = true;

		wadMetaAbort?.abort();
		const ctrl = new AbortController();
		wadMetaAbort = ctrl;

		const pwadIds = pwadIdsRaw.map((s) => s.trim());
		const uniq = normalizeUuidList([iwadId, ...pwadIds]);

		if (uniq.length === 0) {
			resolvedIwad = null;
			resolvedPwads = pwadIds.map(() => null);
			wadMetaLoading = false;
			return;
		}

		try {
			const url = new URL('/api/wad/meta', window.location.origin);
			url.searchParams.set('ids', uniq.join(','));
			const res = await fetch(url, { signal: ctrl.signal });
			if (!res.ok) {
				throw new Error(`Failed to fetch WAD meta: ${res.status}`);
			}
			const payload = (await res.json()) as { items: WadMetaLookupItem[] };
			const byId = new Map<string, WadMetaLookupItem>();
			for (const it of payload.items ?? []) byId.set(it.id.trim().toLowerCase(), it);

			// Ignore stale responses.
			if (token !== wadMetaToken) return;

			const iKey = iwadId.trim().toLowerCase();
			resolvedIwad = iKey
				? byId.get(iKey) ?? { id: iwadId.trim(), ok: false, error: 'Not found' }
				: null;
			resolvedPwads = pwadIds.map((id) => {
				const trimmed = id.trim();
				if (!trimmed) return null;
				const k = trimmed.toLowerCase();
				return byId.get(k) ?? { id: trimmed, ok: false, error: 'Not found' };
			});
		} catch (e) {
			if ((e as { name?: string }).name === 'AbortError') return;
			if (token !== wadMetaToken) return;
			wadMetaError = 'Could not load IWAD/PWAD details.';
		} finally {
			if (token !== wadMetaToken) return;
			wadMetaLoading = false;
		}
	}

	function validate(): boolean {
		nameError = null;
		iwadError = null;
		pwadError = null;
		warpError = null;
		skillError = null;
		maxPlayersError = null;

		if (!serverName.trim()) nameError = 'Server name is required.';
		if (!iwadUuid.trim()) iwadError = 'IWAD UUID is required.';

		if (!warp.trim()) warpError = 'Warp is required.';
		if (!Number.isInteger(skill) || skill < 1 || skill > 5) skillError = 'Skill must be 1..5.';

		if (!singlePlayer) {
			if (!Number.isInteger(maxPlayers) || maxPlayers < 2 || maxPlayers > 64) {
				maxPlayersError = 'Max players must be 2..64.';
			}
		}

		return !(nameError || iwadError || pwadError || warpError || skillError || maxPlayersError);
	}

	async function focusFirst() {
		await tick();
		nameEl?.focus();
	}

	async function onCreate() {
		if (!validate()) return;

		if (singlePlayer) {
			const pwadIds = normalizeUuidList(pwads.map((p) => p.id));
			const u = new URL('https://gib.gg/play/');
			u.searchParams.set('iwad', iwadUuid.trim());
			if (pwadIds.length) u.searchParams.set('pwad', pwadIds.join(','));
			u.searchParams.set('warp', warp.trim());
			u.searchParams.set('skill', String(skill));
			if (browser) window.location.assign(u.toString());
			return;
		}

		showToast('Multiplayer server creation is not wired yet.');
	}

	function getFocusable(container: HTMLElement): HTMLElement[] {
		const selector =
			'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';
		return Array.from(container.querySelectorAll<HTMLElement>(selector)).filter((el) => {
			const style = window.getComputedStyle(el);
			return style.visibility !== 'hidden' && style.display !== 'none';
		});
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

	$effect(() => {
		if (!open) {
			didInit = false;
			wadMetaAbort?.abort();
			wadMetaAbort = null;
			wadMetaLoading = false;
			wadMetaError = null;
			resolvedIwad = null;
			resolvedPwads = [];
			dragKey = null;
			iwadSearchAbort?.abort();
			pwadSearchAbort?.abort();
			iwadSearchLoading = false;
			pwadSearchLoading = false;
			iwadSearchOpen = false;
			pwadSearchOpen = false;
			if (browser) {
				if (iwadSearchTimer != null) window.clearTimeout(iwadSearchTimer);
				if (pwadSearchTimer != null) window.clearTimeout(pwadSearchTimer);
			}
			iwadSearchTimer = null;
			pwadSearchTimer = null;
			return;
		}

		if (!didInit) {
			didInit = true;
			serverName = wadTitle?.trim() ? `${wadTitle.trim()} Server` : 'New Server';
			if (wadIsIwad) {
				iwadUuid = wadId;
				pwads = [];
			} else {
				iwadUuid = DEFAULT_IWAD_UUID_FOR_PWAD;
				pwads = [{ key: makePwadKey(), id: wadId }];
			}
			iwadSearch = '';
			pwadSearch = '';
			warp = maps?.[0]?.map ?? '';
			skill = 3;
			singlePlayer = true;
			maxPlayers = 8;
		}

		if (!browser) return;
		lastActiveEl = document.activeElement as HTMLElement | null;
		void focusFirst();

		const prevOverflow = document.documentElement.style.overflow;
		document.documentElement.style.overflow = 'hidden';
		document.addEventListener('keydown', onKeydown);

		return () => {
			document.removeEventListener('keydown', onKeydown);
			document.documentElement.style.overflow = prevOverflow;
		};
	});

	$effect(() => {
		if (!open) return;
		if (!browser) return;

		const iwad = iwadUuid.trim();
		const pwadIds = pwads.map((p) => p.id);

		// Debounce to avoid firing on every keystroke.
		const t = window.setTimeout(() => {
			void fetchWadMeta(iwad, pwadIds);
		}, 350);
		return () => {
			window.clearTimeout(t);
		};
	});

	$effect(() => {
		if (!open) return;
		if (!browser) return;
		if (!iwadUuid.trim()) return;
		iwadSearchOpen = false;
		iwadSearchItems = [];
		iwadSearchLoading = false;
		iwadSearchAbort?.abort();
		if (iwadSearchTimer != null) window.clearTimeout(iwadSearchTimer);
		iwadSearchTimer = null;
	});

	$effect(() => {
		if (!open) return;
		if (!browser) return;

		const onPointerDown = (e: PointerEvent) => {
			const t = e.target as Node | null;
			if (t && iwadSearchWrap && !iwadSearchWrap.contains(t)) iwadSearchOpen = false;
			if (t && pwadSearchWrap && !pwadSearchWrap.contains(t)) pwadSearchOpen = false;
		};
		document.addEventListener('pointerdown', onPointerDown);
		return () => document.removeEventListener('pointerdown', onPointerDown);
	});

	onDestroy(() => {
		if (!browser) return;
		queueMicrotask(() => lastActiveEl?.focus());
	});
</script>

{#if open}
	<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
		<button
			type="button"
			class="absolute inset-0 bg-zinc-950/80"
			onclick={close}
			aria-label="Close server creation dialog"
		></button>

		<div
			bind:this={modalEl}
			class="relative flex max-h-[85vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
			role="dialog"
			aria-modal="true"
			aria-label="Create server"
			tabindex="-1"
		>
			<div class="flex items-center justify-between border-b border-zinc-800/80 px-4 py-3">
				<div>
					<h2 class="text-base font-semibold tracking-wide text-zinc-100">Create Server</h2>
					<p class="mt-0.5 text-xs text-zinc-400">Configure a new session.</p>
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

			<div class="flex-1 overflow-auto px-4 py-3">
				<div class="grid gap-4">
					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Server Name</h3>
						<input
							bind:this={nameEl}
							value={serverName}
							oninput={(e) => {
								serverName = (e.currentTarget as HTMLInputElement).value;
								nameError = null;
							}}
							class="mt-1.5 w-full rounded-lg bg-zinc-950 px-3 py-1.5 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="My Server"
							autocomplete="off"
						/>
						{#if nameError}
							<p class="mt-2 text-xs text-red-300">{nameError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">IWAD</h3>
						<p class="mt-0.5 text-xs text-zinc-500">Enter the IWAD UUID (base game).</p>
						{#if iwadUuid.trim()}
							<div class="mt-2 flex items-start justify-between gap-2 rounded-lg bg-zinc-900/35 px-3 py-2 ring-1 ring-zinc-800 ring-inset">
								<div class="min-w-0">
									<div class="min-w-0 truncate text-xs text-zinc-200">
										{#if wadMetaLoading}
											<span class="text-zinc-400">Loading…</span>
										{:else}
											{metaLabel(resolvedIwad)}
										{/if}
									</div>
									<div class="mt-0.5 flex flex-wrap gap-x-2 gap-y-0.5 text-[11px] text-zinc-500">
										<span class="font-mono text-zinc-400">{shortId(iwadUuid.trim())}</span>
										{#if !wadMetaLoading}
											<span>{metaSubLabel(resolvedIwad)}</span>
										{/if}
									</div>
								</div>
								<button
									type="button"
									class="shrink-0 cursor-pointer rounded-md bg-transparent px-2.5 py-1 text-xs font-semibold text-zinc-200 ring-1 ring-zinc-800 hover:bg-zinc-900 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
									onclick={() => {
										iwadUuid = '';
										resolvedIwad = null;
										iwadSearch = '';
										iwadSearchOpen = true;
									}}
								>
									Remove
								</button>
							</div>
						{:else}
							<div class="mt-2" bind:this={iwadSearchWrap}>
								<div class="relative">
									<input
										value={iwadSearch}
										oninput={(e) => {
											iwadSearch = (e.currentTarget as HTMLInputElement).value;
											iwadSearchOpen = true;
											iwadError = null;
											scheduleSearch('iwad');
									}}
										onfocus={() => (iwadSearchOpen = true)}
										class="w-full rounded-lg bg-zinc-950 px-3 py-1.5 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
										placeholder="Search IWADs…"
										autocomplete="off"
									/>
									{#if iwadSearchOpen && iwadSearch.trim()}
										<div class="absolute z-50 mt-1 w-full overflow-hidden rounded-lg bg-zinc-950 ring-1 ring-zinc-800">
											<div class="max-h-64 overflow-auto">
												{#if iwadSearchLoading}
													<div class="px-3 py-2 text-xs text-zinc-400">Searching…</div>
												{:else if iwadSearchError}
													<div class="px-3 py-2 text-xs text-red-300">{iwadSearchError}</div>
												{:else if !iwadSearchItems.length}
													<div class="px-3 py-2 text-xs text-zinc-500">Type to search…</div>
												{:else}
													{#each iwadSearchItems as it (it.id)}
														<button
															type="button"
															class="block w-full cursor-pointer px-3 py-2 text-left hover:bg-zinc-900 focus-visible:bg-zinc-900 focus-visible:outline-none"
															onclick={() => {
																iwadUuid = it.id;
																resolvedIwad = null;
																wadMetaError = null;
																iwadSearchOpen = false;
																iwadSearch = '';
														}}
														>
															<div class="truncate text-xs text-zinc-100">{wadLabel(it.meta)}</div>
															<div class="mt-0.5 flex flex-wrap gap-x-2 text-[11px] text-zinc-500">
																<span class="font-mono text-zinc-400">{shortId(it.id)}</span>
																<span>{it.meta.file?.type ?? '—'} • {humanBytes(it.meta.file?.size ?? null)}</span>
															</div>
														</button>
													{/each}
												{/if}
											</div>
										</div>
									{/if}
								</div>
							</div>
						{/if}
						{#if iwadError}
							<p class="mt-2 text-xs text-red-300">{iwadError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">PWADs</h3>
						<div class="mt-1 flex items-center justify-between gap-3">
							<p class="text-xs text-zinc-500">Optional. Drag to reorder (load order matters).</p>
							<div class="text-[11px] text-zinc-600">{pwads.length} selected</div>
						</div>

						{#if pwads.length === 0}
							<div class="mt-2 rounded-lg bg-zinc-900/35 px-3 py-2 text-xs text-zinc-500 ring-1 ring-zinc-800 ring-inset">
								No PWADs selected.
							</div>
						{:else}
							<div class="mt-2 space-y-1.5" role="list">
								{#each pwads as p, idx (p.key)}
									<div
										draggable="true"
										role="listitem"
										ondragstart={(e) => {
											dragKey = p.key;
											try {
												e.dataTransfer?.setData('text/plain', p.key);
												if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
											} catch {
												// ignore
											}
									}}
										ondragover={(e) => {
											e.preventDefault();
											try {
												if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
											} catch {
												// ignore
											}
									}}
										ondrop={(e) => {
											e.preventDefault();
											const from = dragKey ?? e.dataTransfer?.getData('text/plain') ?? '';
											dragKey = null;
											if (from) reorderPwads(from, p.key);
									}}
									class="rounded-lg bg-zinc-900/35 p-1.5 ring-1 ring-zinc-800 ring-inset"
								>
									<div class="grid grid-cols-1 gap-2 sm:grid-cols-[auto_1fr_auto] sm:items-center">
										<div class="flex items-center gap-2">
											<div
												class="select-none rounded-md bg-zinc-950 px-2 py-1 text-xs text-zinc-500 ring-1 ring-zinc-800"
												aria-label="Drag to reorder"
												title="Drag to reorder"
											>
												≡
											</div>
											<div class="text-xs text-zinc-500">#{idx + 1}</div>
										</div>

										<div class="rounded-lg bg-zinc-950/40 px-3 py-1.5 ring-1 ring-zinc-800 ring-inset">
											<div class="min-w-0 truncate text-xs text-zinc-200">
												{#if wadMetaLoading}
													<span class="text-zinc-400">Loading…</span>
												{:else}
													{metaLabel(resolvedPwads[idx] ?? null)}
												{/if}
											</div>
											<div class="mt-0.5 flex flex-wrap gap-x-2 gap-y-0.5 text-[11px] text-zinc-500">
												<span class="font-mono text-zinc-400">{shortId(p.id || (resolvedPwads[idx]?.id ?? ''))}</span>
												{#if !wadMetaLoading}
													<span>{metaSubLabel(resolvedPwads[idx] ?? null)}</span>
												{/if}
											</div>
										</div>

										<button
											type="button"
											class="cursor-pointer rounded-md bg-transparent px-2.5 py-1 text-xs font-semibold text-zinc-200 ring-1 ring-zinc-800 hover:bg-zinc-900 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
											onclick={() => removePwad(p.key)}
											aria-label="Remove PWAD"
										>
											✕
										</button>
									</div>
								</div>
							{/each}
						</div>
					{/if}
						{#if pwadError}
							<p class="mt-2 text-xs text-red-300">{pwadError}</p>
						{/if}
						{#if wadMetaError}
							<p class="mt-2 text-xs text-red-300">{wadMetaError}</p>
						{/if}

						<div class="mt-3" bind:this={pwadSearchWrap}>
							<div class="relative">
								<input
									value={pwadSearch}
									oninput={(e) => {
										pwadSearch = (e.currentTarget as HTMLInputElement).value;
										pwadSearchOpen = true;
										pwadError = null;
										scheduleSearch('pwad');
									}}
									onfocus={() => (pwadSearchOpen = true)}
									class="w-full rounded-lg bg-zinc-950 px-3 py-1.5 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
									placeholder="Search and add PWADs…"
									autocomplete="off"
								/>

								{#if pwadSearchOpen && pwadSearch.trim()}
									<div class="absolute z-50 mt-1 w-full overflow-hidden rounded-lg bg-zinc-950 ring-1 ring-zinc-800">
										<div class="max-h-64 overflow-auto">
											{#if pwadSearchLoading}
												<div class="px-3 py-2 text-xs text-zinc-400">Searching…</div>
											{:else if pwadSearchError}
												<div class="px-3 py-2 text-xs text-red-300">{pwadSearchError}</div>
											{:else if !pwadSearchItems.length}
												<div class="px-3 py-2 text-xs text-zinc-500">Type to search…</div>
											{:else}
												{#each pwadSearchItems as it (it.id)}
													<button
														type="button"
														class="block w-full cursor-pointer px-3 py-2 text-left hover:bg-zinc-900 focus-visible:bg-zinc-900 focus-visible:outline-none"
														onclick={() => {
															addPwadIfMissing(it.id);
															pwadSearch = '';
															pwadSearchOpen = false;
														}}
													>
														<div class="truncate text-xs text-zinc-100">{wadLabel(it.meta)}</div>
														<div class="mt-0.5 flex flex-wrap gap-x-2 text-[11px] text-zinc-500">
															<span class="font-mono text-zinc-400">{shortId(it.id)}</span>
															<span>{it.meta.file?.type ?? '—'} • {humanBytes(it.meta.file?.size ?? null)}</span>
														</div>
													</button>
												{/each}
											{/if}
										</div>
									</div>
								{/if}
							</div>
						</div>
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Warp</h3>
						<div class="mt-2 grid grid-cols-1 gap-2 sm:grid-cols-2">
							<select
								value={warp}
								onchange={(e) => {
									warp = (e.currentTarget as HTMLSelectElement).value;
									warpError = null;
								}}
								class="w-full rounded-lg bg-zinc-950 px-3 py-1.5 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset focus:ring-2 focus:ring-red-700 focus:outline-none"
							>
								{#each maps as m (m.map)}
									<option value={m.map}>{m.title && m.title !== m.map ? `${m.map} — ${m.title}` : m.map}</option>
								{/each}
							</select>
							<input
								value={warp}
								oninput={(e) => {
									warp = (e.currentTarget as HTMLInputElement).value;
									warpError = null;
								}}
								class="w-full rounded-lg bg-zinc-950 px-3 py-1.5 font-mono text-xs text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
								placeholder="MAP01"
								autocomplete="off"
							/>
						</div>
						{#if warpError}
							<p class="mt-2 text-xs text-red-300">{warpError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Skill</h3>
						<select
							value={skill}
							onchange={(e) => {
								skill = Number((e.currentTarget as HTMLSelectElement).value);
								skillError = null;
							}}
							class="mt-1.5 w-full rounded-lg bg-zinc-950 px-3 py-1.5 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset focus:ring-2 focus:ring-red-700 focus:outline-none"
						>
							<option value="1">1 — I'm Too Young to Die</option>
							<option value="2">2 — Hey, Not Too Rough</option>
							<option value="3">3 — Hurt Me Plenty</option>
							<option value="4">4 — Ultra-Violence</option>
							<option value="5">5 — Nightmare!</option>
						</select>
						{#if skillError}
							<p class="mt-2 text-xs text-red-300">{skillError}</p>
						{/if}
					</section>

					<section>
						<div class="flex items-center justify-between gap-4">
							<div>
								<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Single player</h3>
								<p class="mt-0.5 text-xs text-zinc-500">If enabled, Create launches gib.gg/play directly.</p>
							</div>
							<label class="inline-flex items-center gap-2 text-sm text-zinc-200">
								<input
									type="checkbox"
									checked={singlePlayer}
									onchange={(e) => {
										singlePlayer = (e.currentTarget as HTMLInputElement).checked;
										maxPlayersError = null;
									}}
									class="h-4 w-4 rounded border-zinc-700 bg-zinc-950 text-red-600 focus:ring-2 focus:ring-red-700"
								/>
								<span>Enabled</span>
							</label>
						</div>
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Max players</h3>
						<p class="mt-0.5 text-xs text-zinc-500">Disabled for single-player.</p>
						<input
							type="number"
							min="2"
							max="64"
							step="1"
							value={maxPlayers}
							oninput={(e) => {
								maxPlayers = Number((e.currentTarget as HTMLInputElement).value);
								maxPlayersError = null;
							}}
							disabled={singlePlayer}
							class="mt-1.5 w-full rounded-lg bg-zinc-950 px-3 py-1.5 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 disabled:opacity-50 focus:ring-2 focus:ring-red-700 focus:outline-none"
						/>
						{#if maxPlayersError}
							<p class="mt-2 text-xs text-red-300">{maxPlayersError}</p>
						{/if}
					</section>

					<section class="flex items-center justify-end gap-2 border-t border-zinc-800/80 pt-3">
						<button
							type="button"
							class="cursor-pointer rounded-md bg-transparent px-4 py-2 text-sm font-semibold text-zinc-200 ring-1 ring-zinc-800 hover:bg-zinc-900 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
							onclick={close}
						>
							Cancel
						</button>
						<button
							type="button"
							class="cursor-pointer rounded-md bg-red-900/70 px-4 py-2 text-sm font-semibold text-zinc-100 ring-1 ring-red-950/60 hover:bg-red-800/70 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
							onclick={onCreate}
						>
							Create
						</button>
					</section>
				</div>
			</div>
		</div>
	</div>
{/if}
