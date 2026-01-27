<script lang="ts">
	import { marked } from 'marked';
	import manifestoSource from '../../../manifesto.md?raw';

	marked.setOptions({
		gfm: true,
		breaks: false
	});

	const manifestoHtml = marked.parse(manifestoSource) as string;
</script>

<svelte:head>
	<title>Manifesto • ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-10">
	<article
		class="prose prose-invert max-w-none prose-a:break-words prose-a:text-cyan-300 prose-a:no-underline hover:prose-a:underline"
	>
		{@html manifestoHtml}
	</article>
</section>
