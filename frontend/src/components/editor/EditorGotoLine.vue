<template>
  <div
    v-if="modelValue"
    class="absolute top-3 left-1/2 -translate-x-1/2 z-40 bg-white/95 dark:bg-[#111827]/95 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-3 w-72 backdrop-blur-md animate-in slide-in-from-top-2 duration-150 text-xs"
    @click.stop
    @keydown.esc="$emit('update:modelValue', false)"
  >
    <div class="flex items-center justify-between mb-2">
      <span class="font-bold text-gray-900 dark:text-white">Go to Line</span>
      <span class="text-[10px] text-gray-400 font-mono">1 – {{ lineCount }}</span>
    </div>
    <div class="flex items-center space-x-2">
      <input
        ref="inputRef"
        v-model="target"
        @keydown.enter="jump"
        type="text"
        placeholder="e.g. 24 or 24:5"
        class="flex-1 bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none font-mono"
      />
      <button
        @click="jump"
        class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-xl font-bold text-xs cursor-pointer shadow-2xs"
      >
        Go
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue';

const props = defineProps<{
  modelValue: boolean;
  lineCount: number;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void;
  (e: 'jump-to-line', line: number, col: number): void;
}>();

const target = ref('');
const inputRef = ref<HTMLInputElement | null>(null);

function jump() {
  if (!target.value.trim()) return;
  const parts = target.value.split(':');
  const line = parseInt(parts[0], 10);
  const col = parts[1] ? parseInt(parts[1], 10) : 1;

  if (!isNaN(line) && line > 0) {
    emit('jump-to-line', line, col);
    emit('update:modelValue', false);
    target.value = '';
  }
}

function focus() {
  nextTick(() => {
    inputRef.value?.focus();
    inputRef.value?.select();
  });
}

defineExpose({
  focus,
});
</script>
