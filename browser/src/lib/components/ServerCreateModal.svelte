<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { browser } from '$app/environment';
	import { showToast } from '$lib/stores/toast';

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
	let pwadUuids = $state('');
	let warp = $state('');
	let skill = $state<number>(3);
	let singlePlayer = $state(true);
	let maxPlayers = $state<number>(8);

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

	function normalizeList(value: string): string[] {
		const parts = value
			.split(',')
			.map((s) => s.trim())
			.filter((s) => s.length > 0);
		const out: string[] = [];
		const seen = new Set<string>();
		for (const p of parts) {
			const key = p.toLowerCase();
			if (seen.has(key)) continue;
			seen.add(key);
			out.push(p);
		}
		return out;
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
			const pwads = normalizeList(pwadUuids);
			const u = new URL('https://gib.gg/play/');
			u.searchParams.set('iwad', iwadUuid.trim());
			if (pwads.length) u.searchParams.set('pwad', pwads.join(','));
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
			return;
		}

		if (!didInit) {
			didInit = true;
			serverName = wadTitle?.trim() ? `${wadTitle.trim()} Server` : 'New Server';
			if (wadIsIwad) {
				iwadUuid = wadId;
				pwadUuids = '';
			} else {
				iwadUuid = DEFAULT_IWAD_UUID_FOR_PWAD;
				pwadUuids = wadId;
			}
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
			class="relative w-full max-w-2xl overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
			role="dialog"
			aria-modal="true"
			aria-label="Create server"
			tabindex="-1"
		>
			<div class="flex items-center justify-between border-b border-zinc-800/80 px-5 py-4">
				<div>
					<h2 class="text-base font-semibold tracking-wide text-zinc-100">Create Server</h2>
					<p class="mt-1 text-xs text-zinc-400">Configure a new session.</p>
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
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">Server Name</h3>
						<input
							bind:this={nameEl}
							value={serverName}
							oninput={(e) => {
								serverName = (e.currentTarget as HTMLInputElement).value;
								nameError = null;
							}}
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="My Server"
							autocomplete="off"
						/>
						{#if nameError}
							<p class="mt-2 text-xs text-red-300">{nameError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">IWAD</h3>
						<p class="mt-1 text-xs text-zinc-500">Enter the IWAD UUID (base game).</p>
						<input
							value={iwadUuid}
							oninput={(e) => {
								iwadUuid = (e.currentTarget as HTMLInputElement).value;
								iwadError = null;
							}}
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 font-mono text-xs text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="<iwad uuid>"
							autocomplete="off"
						/>
						{#if iwadError}
							<p class="mt-2 text-xs text-red-300">{iwadError}</p>
						{/if}
					</section>

					<section>
						<h3 class="text-xs font-semibold tracking-wide text-zinc-300">PWADs</h3>
						<p class="mt-1 text-xs text-zinc-500">Optional. Comma-separated PWAD UUIDs (this WAD is pre-filled if it’s a PWAD).</p>
						<input
							value={pwadUuids}
							oninput={(e) => {
								pwadUuids = (e.currentTarget as HTMLInputElement).value;
								pwadError = null;
							}}
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 font-mono text-xs text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
							placeholder="<optional pwad uuid>,<optional pwad uuid>"
							autocomplete="off"
						/>
						{#if pwadError}
							<p class="mt-2 text-xs text-red-300">{pwadError}</p>
						{/if}
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
								class="w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset focus:ring-2 focus:ring-red-700 focus:outline-none"
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
								class="w-full rounded-lg bg-zinc-950 px-3 py-2 font-mono text-xs text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
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
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset focus:ring-2 focus:ring-red-700 focus:outline-none"
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
								<p class="mt-1 text-xs text-zinc-500">If enabled, Create launches gib.gg/play directly.</p>
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
						<p class="mt-1 text-xs text-zinc-500">Disabled for single-player.</p>
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
							class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 disabled:opacity-50 focus:ring-2 focus:ring-red-700 focus:outline-none"
						/>
						{#if maxPlayersError}
							<p class="mt-2 text-xs text-red-300">{maxPlayersError}</p>
						{/if}
					</section>

					<section class="flex items-center justify-end gap-2 border-t border-zinc-800/80 pt-4">
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
