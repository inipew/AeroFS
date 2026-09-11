<template>
  <div
    :class="[
      'h-full overflow-y-auto p-4 sm:p-6 bg-white dark:bg-[#0b0f19] text-gray-900 dark:text-slate-100 prose dark:prose-invert max-w-none text-xs sm:text-sm border-t md:border-t-0 scrollbar-thin',
      isMobile ? 'w-full' : 'w-1/2'
    ]"
    v-html="renderedMarkdown"
  ></div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  content: string;
  isMobile: boolean;
}>();

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

const renderedMarkdown = computed(() => {
  if (!props.content) return '';
  let text = escapeHtml(props.content);

  // 1. Fenced Code Blocks (```lang ... ```)
  text = text.replace(/```([a-zA-Z0-9_-]*)\n([\s\S]*?)```/gim, (_match, lang, code) => {
    const langBadge = lang
      ? `<div class="text-[10px] text-gray-400 font-mono pb-1 mb-1 border-b border-gray-200/40 dark:border-slate-800">${lang}</div>`
      : '';
    return `<pre class="bg-gray-100 dark:bg-[#060910] p-3 rounded-xl font-mono text-xs overflow-x-auto my-2 border border-gray-200/80 dark:border-slate-800 text-gray-800 dark:text-slate-200 leading-relaxed">${langBadge}<code>${code.trim()}</code></pre>`;
  });

  // 2. Headers, Callouts, and Formatting
  text = text
    .replace(/^### (.*$)/gim, '<h3 class="text-base font-bold mb-1.5 mt-3 text-gray-900 dark:text-white">$1</h3>')
    .replace(/^## (.*$)/gim, '<h2 class="text-lg font-bold border-b border-gray-200 dark:border-slate-800 pb-1 mb-2 mt-4 text-gray-900 dark:text-white">$1</h2>')
    .replace(/^# (.*$)/gim, '<h1 class="text-xl font-bold border-b border-gray-200 dark:border-slate-800 pb-1 mb-2 text-gray-900 dark:text-white">$1</h1>')
    .replace(/^\> \[!NOTE\](.*$)/gim, '<div class="border-l-4 border-blue-500 pl-3 py-2 bg-blue-50/30 dark:bg-blue-950/30 text-blue-900 dark:text-blue-300 my-2 rounded-r font-medium">ℹ️ <strong>Note:</strong> $1</div>')
    .replace(/^\> \[!TIP\](.*$)/gim, '<div class="border-l-4 border-emerald-500 pl-3 py-2 bg-emerald-50/30 dark:bg-emerald-950/30 text-emerald-900 dark:text-emerald-300 my-2 rounded-r font-medium">💡 <strong>Tip:</strong> $1</div>')
    .replace(/^\> \[!WARNING\](.*$)/gim, '<div class="border-l-4 border-amber-500 pl-3 py-2 bg-amber-50/30 dark:bg-amber-950/30 text-amber-900 dark:text-amber-300 my-2 rounded-r font-medium">⚠️ <strong>Warning:</strong> $1</div>')
    .replace(/^\> (.*$)/gim, '<blockquote class="border-l-4 border-gray-300 dark:border-slate-700 pl-3 py-1 text-gray-600 dark:text-slate-400 italic my-2 bg-gray-50/40 dark:bg-slate-900/40 rounded-r">$1</blockquote>')
    .replace(/\*\*(.*?)\*\*/gim, '<strong class="font-bold text-gray-900 dark:text-white">$1</strong>')
    .replace(/\*(.*?)\*/gim, '<em class="italic">$1</em>')
    .replace(/`([^`\n]+)`/gim, '<code class="bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 rounded font-mono text-blue-600 dark:text-blue-400 text-xs">$1</code>')
    .replace(/\n$/gim, '<br />')
    .replace(/\n/gim, '<br />');

  return text;
});
</script>
