<template>
  <div
    v-if="conflictState?.isOpen"
    class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4 select-none font-sans text-xs animate-in fade-in duration-150"
  >
    <div
      class="bg-white dark:bg-[#0c101c] border border-gray-200 dark:border-slate-800 rounded-2xl max-w-md w-full p-5 shadow-2xl space-y-4 animate-in zoom-in-95 duration-150"
      @click.stop
    >
      <!-- Header -->
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-amber-500/10 text-amber-500 flex items-center justify-center shrink-0">
          <FbIcon name="info" size="18px" />
        </div>
        <div class="truncate">
          <h3 class="text-sm font-bold text-gray-900 dark:text-slate-100">File Already Exists</h3>
          <p class="text-[11px] text-gray-500 dark:text-slate-400 font-mono truncate">
            {{ conflictState.fileName }}
          </p>
        </div>
      </div>

      <p class="text-xs text-gray-600 dark:text-slate-300">
        A file with this name already exists in the destination folder. How would you like to proceed?
      </p>

      <!-- Resolution Choices -->
      <div class="space-y-2">
        <label
          :class="[
            'p-3 rounded-xl border flex items-start space-x-3 cursor-pointer transition select-none',
            selectedResolution === 'replace'
              ? 'bg-blue-50/70 dark:bg-blue-950/30 border-blue-500/50 ring-1 ring-blue-500/30'
              : 'bg-gray-50/50 dark:bg-slate-900/50 border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700'
          ]"
        >
          <input
            type="radio"
            value="replace"
            v-model="selectedResolution"
            class="mt-0.5 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <div class="space-y-0.5">
            <span class="font-bold text-gray-900 dark:text-slate-100 text-xs">Replace</span>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              Overwrite the existing destination file.
            </p>
          </div>
        </label>

        <label
          :class="[
            'p-3 rounded-xl border flex items-start space-x-3 cursor-pointer transition select-none',
            selectedResolution === 'keep_both'
              ? 'bg-blue-50/70 dark:bg-blue-950/30 border-blue-500/50 ring-1 ring-blue-500/30'
              : 'bg-gray-50/50 dark:bg-slate-900/50 border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700'
          ]"
        >
          <input
            type="radio"
            value="keep_both"
            v-model="selectedResolution"
            class="mt-0.5 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <div class="space-y-0.5">
            <span class="font-bold text-gray-900 dark:text-slate-100 text-xs">Keep Both (Auto-Rename)</span>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              Append a number suffix (e.g. <span class="font-mono">{{ suggestedNewName }}</span>) to preserve both copies.
            </p>
          </div>
        </label>

        <label
          :class="[
            'p-3 rounded-xl border flex items-start space-x-3 cursor-pointer transition select-none',
            selectedResolution === 'skip'
              ? 'bg-blue-50/70 dark:bg-blue-950/30 border-blue-500/50 ring-1 ring-blue-500/30'
              : 'bg-gray-50/50 dark:bg-slate-900/50 border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700'
          ]"
        >
          <input
            type="radio"
            value="skip"
            v-model="selectedResolution"
            class="mt-0.5 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <div class="space-y-0.5">
            <span class="font-bold text-gray-900 dark:text-slate-100 text-xs">Skip</span>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              Do not copy this file and move on to the next item.
            </p>
          </div>
        </label>
      </div>

      <!-- Apply to all checkbox -->
      <div class="pt-1 flex items-center space-x-2">
        <input
          id="applyToAll"
          type="checkbox"
          v-model="applyToAll"
          class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
        />
        <label for="applyToAll" class="text-xs text-gray-700 dark:text-slate-300 cursor-pointer select-none font-medium">
          Apply to all remaining conflicts in this transfer
        </label>
      </div>

      <!-- Action buttons -->
      <div class="flex items-center justify-between pt-3 border-t border-gray-100 dark:border-slate-800/80">
        <button
          type="button"
          @click="handleCancel"
          class="px-3.5 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
        >
          Cancel Transfer
        </button>

        <button
          type="button"
          @click="handleContinue"
          class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-semibold shadow-xs transition text-xs cursor-pointer"
        >
          Continue
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useTransferStore, type ConflictResolution } from '../../stores/transferStore';

const transferStore = useTransferStore();
const conflictState = computed(() => transferStore.conflictState);

const selectedResolution = ref<ConflictResolution>('replace');
const applyToAll = ref(false);

const suggestedNewName = computed(() => {
  const name = conflictState.value?.fileName || 'file';
  const dotIdx = name.lastIndexOf('.');
  if (dotIdx > 0) {
    return `${name.substring(0, dotIdx)} (1)${name.substring(dotIdx)}`;
  }
  return `${name} (1)`;
});

function handleContinue() {
  if (conflictState.value?.resolve) {
    conflictState.value.resolve(selectedResolution.value, applyToAll.value);
  }
  transferStore.resolveConflict(selectedResolution.value, applyToAll.value);
}

function handleCancel() {
  if (conflictState.value?.resolve) {
    conflictState.value.resolve('cancel', false);
  }
  transferStore.resolveConflict('cancel', false);
}
</script>
