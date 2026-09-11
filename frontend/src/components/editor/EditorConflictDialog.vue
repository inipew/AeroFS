<template>
  <div>
    <!-- Unsaved Changes Confirmation Modal -->
    <div
      v-if="isUnsavedConfirmOpen"
      class="fixed inset-0 z-60 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150"
      @click="$emit('cancel-unsaved')"
    >
      <div
        class="bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-3xl p-5 sm:p-6 max-w-md w-full shadow-2xl space-y-4 animate-in zoom-in-95 duration-150 text-gray-900 dark:text-slate-100"
        @click.stop
      >
        <div class="flex items-start space-x-3.5">
          <div class="w-11 h-11 rounded-2xl bg-amber-500/10 dark:bg-amber-500/20 text-amber-500 flex items-center justify-center shrink-0 ring-1 ring-amber-500/30">
            <span class="text-xl">⚠️</span>
          </div>
          <div>
            <h3 class="font-bold text-base text-gray-900 dark:text-white">Unsaved Changes</h3>
            <p class="text-xs text-gray-500 dark:text-slate-400 mt-1 leading-relaxed">
              Do you want to save the changes made to
              <span class="font-bold text-gray-800 dark:text-slate-200 font-mono">{{ filename }}</span>
              before closing?
            </p>
            <p class="text-[11px] text-amber-600 dark:text-amber-400 mt-1 font-medium">
              Your changes will be lost if you choose "Don't Save".
            </p>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2 pt-2 border-t border-gray-100 dark:border-slate-800/80">
          <button
            @click="$emit('cancel-unsaved')"
            class="px-4 py-2.5 rounded-xl bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-300 font-semibold text-xs transition cursor-pointer order-3 sm:order-1"
          >
            Cancel
          </button>

          <button
            @click="$emit('discard-unsaved')"
            class="px-4 py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 border border-red-500/20 font-semibold text-xs transition cursor-pointer order-2"
          >
            Don't Save
          </button>

          <button
            @click="$emit('save-and-close')"
            :disabled="saving"
            class="px-4 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white font-bold text-xs flex items-center justify-center space-x-1.5 shadow-md shadow-blue-600/20 transition cursor-pointer order-1 sm:order-3 disabled:opacity-50"
          >
            <FbIcon name="save" size="13px" :class="{ 'animate-spin': saving }" />
            <span>{{ saving ? 'Saving...' : 'Save & Close' }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Server Conflict Detected Modal -->
    <div
      v-if="isConflictModalOpen"
      class="fixed inset-0 z-60 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150"
      @click="$emit('cancel-conflict')"
    >
      <div
        class="bg-white dark:bg-[#0f1422] border border-red-500/30 rounded-3xl p-5 sm:p-6 max-w-md w-full shadow-2xl space-y-4 animate-in zoom-in-95 duration-150 text-gray-900 dark:text-slate-100 ring-1 ring-red-500/20"
        @click.stop
      >
        <div class="flex items-start space-x-3.5">
          <div class="w-11 h-11 rounded-2xl bg-red-500/10 dark:bg-red-500/20 text-red-500 flex items-center justify-center shrink-0 ring-1 ring-red-500/30">
            <span class="text-xl">⚠️</span>
          </div>
          <div>
            <h3 class="font-bold text-base text-gray-900 dark:text-white">File Conflict Detected</h3>
            <p class="text-xs text-gray-500 dark:text-slate-400 mt-1 leading-relaxed">
              <span class="font-bold text-gray-800 dark:text-slate-200 font-mono">{{ filename }}</span>
              was modified on disk since you opened it.
            </p>
            <p class="text-[11px] text-red-600 dark:text-red-400 mt-1 font-medium">
              Overwriting will replace remote changes on disk with your current editor content.
            </p>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2 pt-2 border-t border-gray-100 dark:border-slate-800/80">
          <button
            @click="$emit('cancel-conflict')"
            class="px-4 py-2.5 rounded-xl bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-300 font-semibold text-xs transition cursor-pointer order-3 sm:order-1"
          >
            Cancel
          </button>

          <button
            @click="$emit('reload-disk')"
            :disabled="saving"
            class="px-4 py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 border border-red-500/20 font-semibold text-xs transition cursor-pointer order-2 disabled:opacity-50"
          >
            <span>Discard & Reload Disk</span>
          </button>

          <button
            @click="$emit('force-save')"
            :disabled="saving"
            class="px-4 py-2.5 rounded-xl bg-amber-600 hover:bg-amber-700 text-white font-bold text-xs flex items-center justify-center space-x-1.5 shadow-md shadow-amber-600/20 transition cursor-pointer order-1 sm:order-3 disabled:opacity-50"
          >
            <span>Overwrite Disk</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import FbIcon from '../common/FbIcon.vue';

defineProps<{
  filename: string;
  isUnsavedConfirmOpen: boolean;
  isConflictModalOpen: boolean;
  saving: boolean;
}>();

defineEmits<{
  (e: 'cancel-unsaved'): void;
  (e: 'discard-unsaved'): void;
  (e: 'save-and-close'): void;
  (e: 'cancel-conflict'): void;
  (e: 'force-save'): void;
  (e: 'reload-disk'): void;
}>();
</script>
