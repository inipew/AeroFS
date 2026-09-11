<template>
  <div class="relative flex items-center flex-1 min-w-0">
    <form @submit.prevent="handleSubmit" class="w-full flex items-center min-w-0">
      <input
        ref="inputRef"
        v-model="text"
        type="text"
        placeholder="/path/to/folder"
        @keydown.esc.stop="handleCancel"
        @keydown.tab.prevent="autocompleteFirst"
        @blur="handleBlur"
        class="w-full bg-white dark:bg-[#0f1422] border border-blue-500 rounded-xl px-2.5 py-1 text-xs font-mono text-gray-800 dark:text-slate-100 outline-none focus:ring-2 focus:ring-blue-500/30 shadow-xs"
      />
    </form>

    <!-- Confirm / Cancel buttons -->
    <div class="flex items-center space-x-0.5 ml-1 shrink-0">
      <button
        type="button"
        @mousedown.prevent="handleSubmit"
        class="p-1 text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-950/40 rounded-lg transition cursor-pointer text-xs font-bold"
        title="Go to Path (Enter)"
      >
        ✓
      </button>
      <button
        type="button"
        @mousedown.prevent="handleCancel"
        class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg transition cursor-pointer text-xs"
        title="Cancel (Esc)"
      >
        ✕
      </button>
    </div>

    <!-- Autocomplete Dropdown -->
    <div
      v-if="filteredSuggestions.length > 0"
      class="absolute top-full left-0 mt-1 w-full bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl shadow-xl z-50 py-1 max-h-40 overflow-y-auto text-[11px] font-mono"
    >
      <div
        v-for="sug in filteredSuggestions"
        :key="sug"
        @mousedown.prevent="applySuggestion(sug)"
        class="px-2.5 py-1 hover:bg-blue-50 dark:hover:bg-blue-950/40 text-gray-700 dark:text-slate-300 hover:text-blue-600 dark:hover:text-blue-400 cursor-pointer flex items-center space-x-1.5 truncate"
      >
        <span class="text-xs">📁</span>
        <span class="truncate">{{ sug }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue';

const props = defineProps<{
  initialPath: string;
  suggestions?: string[];
}>();

const emit = defineEmits<{
  (e: 'submit', path: string): void;
  (e: 'cancel'): void;
}>();

const text = ref(props.initialPath || '/');
const inputRef = ref<HTMLInputElement | null>(null);

const filteredSuggestions = computed(() => {
  if (!props.suggestions || !text.value) return [];
  const current = text.value.toLowerCase();
  return props.suggestions
    .filter((s) => s.toLowerCase().includes(current))
    .slice(0, 5);
});

onMounted(async () => {
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();
});

function handleSubmit() {
  const trimmed = text.value.trim();
  if (trimmed) {
    const formatted = trimmed.startsWith('/') ? trimmed : `/${trimmed}`;
    emit('submit', formatted);
  } else {
    emit('cancel');
  }
}

function handleCancel() {
  emit('cancel');
}

function applySuggestion(sug: string) {
  text.value = sug;
  handleSubmit();
}

function autocompleteFirst() {
  if (filteredSuggestions.value.length > 0) {
    applySuggestion(filteredSuggestions.value[0]);
  }
}

function handleBlur() {
  setTimeout(() => {
    emit('cancel');
  }, 180);
}
</script>
