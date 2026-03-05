<script lang="ts">
	import { marked } from 'marked';
	import privacyPolicySource from '../../../../legal/privacy-policy.md?raw';

	marked.setOptions({
		gfm: true,
		breaks: false
	});

	const privacyPolicyHtml = marked.parse(privacyPolicySource) as string;
</script>

<svelte:head>
	<title>Privacy Policy • ɢɪʙ.ɢɢ</title>
</svelte:head>

<section class="mx-auto w-full max-w-6xl px-4 py-10">
	<article
		class="manifesto-contrast prose prose-invert max-w-none prose-a:break-words prose-a:text-cyan-300 prose-a:no-underline hover:prose-a:underline"
	>
		{@html privacyPolicyHtml}
	</article>
</section>
