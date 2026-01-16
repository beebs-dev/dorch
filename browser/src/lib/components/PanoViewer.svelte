<script lang="ts">
	import { onMount } from 'svelte';

	type Props = {
		url: string;
		pitchLimitDeg?: number;
	};

	let { url, pitchLimitDeg = 10 }: Props = $props();

	let canvas: HTMLCanvasElement | null = null;
	let error = $state<string | null>(null);

	onMount(() => {
		let stop = false;
		let raf = 0;
		let cleanup: (() => void) | null = null;

		(async () => {
			if (!canvas) return;

			const [THREE, controlsMod] = await Promise.all([
				import('three'),
				import('three/examples/jsm/controls/OrbitControls.js')
			]);
			const { OrbitControls } = controlsMod as unknown as {
				OrbitControls: new (...args: any[]) => any;
			};

			const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
			renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio ?? 1, 2));

			const scene = new THREE.Scene();
			const camera = new THREE.PerspectiveCamera(75, 2, 0.1, 2000);
			camera.position.set(0, 0, 0.01);

			const controls = new OrbitControls(camera, renderer.domElement);
			controls.enablePan = false;
			controls.rotateSpeed = 0.35;
			controls.enableDamping = true;
			controls.dampingFactor = 0.08;

			const pitchLimit = THREE.MathUtils.degToRad(pitchLimitDeg);
			const horizon = Math.PI / 2;
			controls.minPolarAngle = horizon - pitchLimit;
			controls.maxPolarAngle = horizon + pitchLimit;

			controls.enableZoom = true;
			controls.minDistance = 0.01;
			controls.maxDistance = 0.01;

			const DEFAULT_FOV = 75;
			camera.fov = DEFAULT_FOV;
			camera.updateProjectionMatrix();

			function resizeRendererToDisplaySize() {
				if (!canvas) return;
				const w = canvas.clientWidth;
				const h = canvas.clientHeight;
				const pr = renderer.getPixelRatio();
				const needResize =
					canvas.width !== Math.floor(w * pr) || canvas.height !== Math.floor(h * pr);
				if (needResize) {
					renderer.setSize(w, h, false);
					camera.aspect = w / h;
					camera.updateProjectionMatrix();
				}
			}

			function onWheel(e: WheelEvent) {
				e.preventDefault();
				const delta = Math.sign(e.deltaY);
				camera.fov = THREE.MathUtils.clamp(camera.fov + delta * 2.5, 35, 95);
				camera.updateProjectionMatrix();
			}

			function onDblClick() {
				controls.reset();
				camera.fov = DEFAULT_FOV;
				camera.updateProjectionMatrix();
			}

			globalThis.addEventListener('wheel', onWheel, { passive: false });
			globalThis.addEventListener('dblclick', onDblClick);

			const loader = new THREE.TextureLoader();
			let geometry: any = null;
			let material: any = null;
			let mesh: any = null;
			let texture: any = null;
			let pngObjectUrl: string | null = null;

			async function webpUrlToPngObjectUrl(webpUrl: string): Promise<string> {
				const response = await fetch(webpUrl);
				if (!response.ok) {
					throw new Error(`Failed to fetch (${response.status}): ${webpUrl}`);
				}
				const webpBlob = await response.blob();

				let bitmap: ImageBitmap | HTMLImageElement;
				try {
					bitmap = await createImageBitmap(webpBlob);
				} catch {
					const webpObjectUrl = URL.createObjectURL(webpBlob);
					try {
						const img = new Image();
						img.decoding = 'async';
						img.src = webpObjectUrl;
						if (img.decode) await img.decode();
						else {
							await new Promise<void>((resolve, reject) => {
								img.onload = () => resolve();
								img.onerror = () => reject(new Error('Failed to decode image'));
							});
						}
						bitmap = img;
					} finally {
						URL.revokeObjectURL(webpObjectUrl);
					}
				}

				const c = document.createElement('canvas');
				c.width = bitmap.width;
				c.height = bitmap.height;
				const ctx = c.getContext('2d', { alpha: false });
				if (!ctx) throw new Error('Failed to get 2D canvas context');
				ctx.drawImage(bitmap as any, 0, 0);

				const pngBlob = await new Promise<Blob>((resolve, reject) => {
					c.toBlob((b) => (b ? resolve(b) : reject(new Error('canvas.toBlob returned null'))), 'image/png');
				});

				if (typeof (bitmap as any).close === 'function') {
					(bitmap as any).close();
				}

				return URL.createObjectURL(pngBlob);
			}

			try {
				pngObjectUrl = await webpUrlToPngObjectUrl(url);
				texture = await loader.loadAsync(pngObjectUrl);
				texture.colorSpace = THREE.SRGBColorSpace;
				texture.wrapS = THREE.RepeatWrapping;
				texture.repeat.x = -1;
				texture.flipY = false;
				texture.needsUpdate = true;

				geometry = new THREE.SphereGeometry(500, 64, 32);
				geometry.scale(-1, 1, 1);

				material = new THREE.MeshBasicMaterial({ map: texture });
				mesh = new THREE.Mesh(geometry, material);
				scene.add(mesh);
			} catch (e) {
				error = e instanceof Error ? e.message : 'Failed to load panorama';
			}

			cleanup = () => {
				globalThis.removeEventListener('wheel', onWheel as any);
				globalThis.removeEventListener('dblclick', onDblClick as any);

				try {
					controls.dispose();
				} catch {
					// ignore
				}
				try {
					material?.dispose?.();
					geometry?.dispose?.();
					texture?.dispose?.();
					renderer.dispose();
				} catch {
					// ignore
				}

				if (pngObjectUrl) URL.revokeObjectURL(pngObjectUrl);
			};

			const animate = () => {
				if (stop) return;
				resizeRendererToDisplaySize();
				controls.update();
				renderer.render(scene, camera);
				raf = requestAnimationFrame(animate);
			};
			animate();
		})();

		return () => {
			stop = true;
			if (raf) cancelAnimationFrame(raf);
			cleanup?.();
		};
	});
</script>

<div class="overflow-hidden rounded-xl ring-1 ring-inset ring-zinc-800">
	<div class="aspect-[16/9] bg-zinc-900">
		<canvas bind:this={canvas} class="h-full w-full cursor-grab active:cursor-grabbing"></canvas>
	</div>
	{#if error}
		<div class="border-t border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-300">
			{error}
		</div>
	{/if}
</div>
