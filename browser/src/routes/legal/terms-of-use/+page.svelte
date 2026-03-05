<script lang="ts">
	import { marked } from 'marked';
	import termsSource from '../../../../legal/terms-of-use.md?raw';

	marked.setOptions({
		gfm: true,
		breaks: false
	});

	const termsHtml = marked.parse(termsSource) as string;
</script>

<svelte:head>
	<title>Terms of Use • ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-10">
	<article
		class="manifesto-contrast prose prose-invert max-w-none prose-a:break-words prose-a:text-cyan-300 prose-a:no-underline hover:prose-a:underline"
	>
		{@html termsHtml}
	</article>
</section>
