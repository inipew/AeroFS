<template>
  <div
    v-if="modelValue"
    class="absolute top-3 right-3 z-40 bg-white/95 dark:bg-[#111827]/95 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-2.5 flex flex-col gap-2 max-w-sm sm:max-w-md w-full backdrop-blur-md animate-in slide-in-from-top-2 duration-150"
    @click.stop
    @keydown.esc="$emit('update:modelValue', false)"
  >
    <!-- Search Input Row -->
    <div class="flex items-center space-x-1.5">
      <div class="relative flex-1">
        <input
          ref="searchInputRef"
          :value="searchQuery"
          @input="$emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
          @keydown.enter="$emit('find-next')"
          @keydown.shift.enter.prevent="$emit('find-prev')"
          type="text"
          placeholder="Find in file..."
          class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none pr-16 font-mono"
        />
        <span class="absolute right-2 top-1/2 -translate-y-1/2 text-[10px] text-gray-400 font-mono">
          {{ matchCount > 0 ? `${currentMatchIdx}/${matchCount}` : (searchQuery ? '0/0' : '') }}
        </span>
      </div>

      <!-- Match Navigation Buttons -->
      <button
        @click="$emit('find-prev')"
        :disabled="matchCount === 0"
        class="p-1.5 rounded-lg border border-gray-200 dark:border-slate-700 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 text-gray-600 dark:text-slate-300 cursor-pointer"
        title="Previous Match (Shift+Enter)"
      >
        <FbIcon name="arrow-up" size="12px" />
      </button>
      <button
        @click="$emit('find-next')"
        :disabled="matchCount === 0"
        class="p-1.5 rounded-lg border border-gray-200 dark:border-slate-700 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 text-gray-600 dark:text-slate-300 cursor-pointer"
        title="Next Match (Enter)"
      >
        <FbIcon name="arrow-down" size="12px" />
      </button>

      <!-- Case Sensitive Toggle -->
      <button
        @click="$emit('toggle-case-sensitive')"
        :class="[
          'px-1.5 py-1 rounded-lg border font-mono text-[10px] font-bold cursor-pointer transition',
          searchCaseSensitive
            ? 'bg-blue-600 text-white border-blue-600'
            : 'border-gray-200 dark:border-slate-700 text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        title="Match Case (Aa)"
      >
        Aa
      </button>

      <!-- Whole Word Toggle -->
      <button
        @click="$emit('toggle-whole-word')"
        :class="[
          'px-1.5 py-1 rounded-lg border font-mono text-[10px] font-bold cursor-pointer transition',
          searchWholeWord
            ? 'bg-blue-600 text-white border-blue-600'
            : 'border-gray-200 dark:border-slate-700 text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        title="Match Whole Word (\b)"
      >
        \b
      </button>

      <!-- Regex Toggle -->
      <button
        @click="$emit('toggle-regex')"
        :class="[
          'px-1.5 py-1 rounded-lg border font-mono text-[10px] font-bold cursor-pointer transition',
          searchRegex
            ? 'bg-blue-600 text-white border-blue-600'
            : 'border-gray-200 dark:border-slate-700 text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        title="Regular Expression (.*)"
      >
        .*
      </button>

      <!-- Close Search Bar -->
      <button
        @click="$emit('update:modelValue', false)"
        class="p-1.5 text-gray-400 hover:text-gray-700 dark:hover:text-white rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 cursor-pointer"
        title="Close Search (Esc)"
      >
        ✕
      </button>
    </div>

    <!-- Replace Input Row -->
    <div class="flex items-center space-x-1.5">
      <input
        ref="replaceInputRef"
        :value="replaceQuery"
        @input="$emit('update:replaceQuery', ($event.target as HTMLInputElement).value)"
        @keydown.enter="$emit('replace-current')"
        type="text"
        placeholder="Replace with..."
        class="flex-1 bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none font-mono"
      />
      <button
        @click="$emit('replace-current')"
        :disabled="matchCount === 0"
        class="px-2.5 py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 text-gray-700 dark:text-slate-200 text-xs font-semibold cursor-pointer shadow-2xs"
      >
        Replace
      </button>
      <button
        @click="$emit('replace-all')"
        :disabled="matchCount === 0"
        class="px-2.5 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-700 text-white disabled:opacity-40 text-xs font-semibold cursor-pointer shadow-2xs"
      >
        All
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';

defineProps<{
  modelValue: boolean;
  searchQuery: string;
  replaceQuery: string;
  searchCaseSensitive: boolean;
  searchWholeWord: boolean;
  searchRegex: boolean;
  matchCount: number;
  currentMatchIdx: number;
}>();

defineEmits<{
  (e: 'update:modelValue', val: boolean): void;
  (e: 'update:searchQuery', val: string): void;
  (e: 'update:replaceQuery', val: string): void;
  (e: 'toggle-case-sensitive'): void;
  (e: 'toggle-whole-word'): void;
  (e: 'toggle-regex'): void;
  (e: 'find-next'): void;
  (e: 'find-prev'): void;
  (e: 'replace-current'): void;
  (e: 'replace-all'): void;
}>();

const searchInputRef = ref<HTMLInputElement | null>(null);
const replaceInputRef = ref<HTMLInputElement | null>(null);

function focusSearch() {
  nextTick(() => {
    searchInputRef.value?.focus();
    searchInputRef.value?.select();
  });
}

function focusReplace() {
  nextTick(() => {
    replaceInputRef.value?.focus();
    replaceInputRef.value?.select();
  });
}

defineExpose({
  focusSearch,
  focusReplace,
});
</script>
