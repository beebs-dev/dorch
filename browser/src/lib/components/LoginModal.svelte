<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { browser } from '$app/environment';
	import { showToast } from '$lib/stores/toast';

	let { open, onClose }: { open: boolean; onClose: () => void } = $props();

	let emailOrUsername = $state('');
	let password = $state('');
	let rememberMe = $state(true);

	let modalEl = $state<HTMLDivElement | null>(null);
	let emailEl = $state<HTMLInputElement | null>(null);

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
		emailEl?.focus();
	}

	function close() {
		onClose();
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
		if (!browser) return;
		if (!open) return;

		lastActiveEl = document.activeElement;
		focusFirst();

		const onDocKeydown = (e: KeyboardEvent) => onKeydown(e);
		document.addEventListener('keydown', onDocKeydown);

		const prevOverflow = document.documentElement.style.overflow;
		document.documentElement.style.overflow = 'hidden';

		return () => {
			document.removeEventListener('keydown', onDocKeydown);
			document.documentElement.style.overflow = prevOverflow;
		};
	});

	onDestroy(() => {
		if (!browser) return;
		queueMicrotask(() => lastActiveEl?.focus());
	});

	function onSubmit(e: SubmitEvent) {
		e.preventDefault();
		showToast('Login is not wired up yet (UI only).');
		close();
	}

	function comingSoon(message: string) {
		showToast(message);
	}
</script>

{#if open}
	<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
		<button
			type="button"
			class="absolute inset-0 bg-zinc-950/80"
			onclick={close}
			aria-label="Close login dialog"
		></button>

		<div
			bind:this={modalEl}
			class="relative w-full max-w-md overflow-hidden rounded-xl bg-zinc-950 ring-1 ring-zinc-800 ring-inset"
			role="dialog"
			aria-modal="true"
			aria-label="Login"
			tabindex="-1"
		>
			<div class="flex items-center justify-between border-b border-zinc-800/80 px-5 py-4">
				<div>
					<h2 class="text-base font-semibold tracking-wide text-zinc-100">Sign in</h2>
					<p class="mt-1 text-xs text-zinc-400">Use your GIB.GG account.</p>
				</div>
				<button
					type="button"
					class="rounded-md p-2 text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-100 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
					onclick={close}
					aria-label="Close"
				>
					<span aria-hidden="true">✕</span>
				</button>
			</div>

			<form class="px-5 py-4" onsubmit={onSubmit}>
				<label class="block text-xs font-semibold tracking-wide text-zinc-300">
					Email or username
					<input
						bind:this={emailEl}
						bind:value={emailOrUsername}
						autocomplete="username"
						class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
						placeholder="you@example.com"
						required
					/>
				</label>

				<label class="mt-4 block text-xs font-semibold tracking-wide text-zinc-300">
					Password
					<input
						bind:value={password}
						autocomplete="current-password"
						type="password"
						class="mt-2 w-full rounded-lg bg-zinc-950 px-3 py-2 text-sm text-zinc-100 ring-1 ring-zinc-800 ring-inset placeholder:text-zinc-600 focus:ring-2 focus:ring-red-700 focus:outline-none"
						placeholder="••••••••"
						required
					/>
				</label>

				<div class="mt-4 flex items-center justify-between gap-4">
					<label class="inline-flex items-center gap-2 text-xs text-zinc-300">
						<input
							type="checkbox"
							bind:checked={rememberMe}
							class="h-4 w-4 rounded border-zinc-700 bg-zinc-950 text-red-600 focus:ring-red-700"
						/>
						Remember me
					</label>

					<button
						type="button"
						class="text-xs font-semibold text-red-300 hover:text-red-200 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none rounded"
						onclick={() => comingSoon('Password reset is coming soon.')}
					>
						Forgot your password?
					</button>
				</div>

				<button
					type="submit"
					class="mt-5 w-full rounded-lg bg-red-700 px-4 py-2 text-sm font-semibold text-white transition hover:bg-red-600 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none"
				>
					Sign in
				</button>

				<div class="mt-4 text-center text-xs text-zinc-400">
					Don’t have an account?
					<button
						type="button"
						class="ml-1 font-semibold text-red-300 hover:text-red-200 focus-visible:ring-2 focus-visible:ring-zinc-500 focus-visible:outline-none rounded"
						onclick={() => comingSoon('Sign up is coming soon.')}
					>
						Sign up
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}
