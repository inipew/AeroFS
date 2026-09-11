<template>
  <Transition name="fade">
    <div
      v-if="isOpen"
      class="absolute inset-0 z-40 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 select-none text-xs"
      @click.self="$emit('close')"
    >
      <div
        class="bg-white dark:bg-[#151922] border border-gray-200 dark:border-white/[0.1] rounded-2xl sm:rounded-3xl max-w-md w-full p-5 sm:p-6 shadow-2xl space-y-4 animate-in fade-in zoom-in-95 duration-150"
        @click.stop
      >
        <!-- Title -->
        <div class="flex items-center justify-between">
          <div class="flex items-center space-x-2.5">
            <div class="w-9 h-9 rounded-xl bg-blue-500/15 text-blue-600 dark:text-blue-400 flex items-center justify-center">
              <FbIcon name="download" size="18px" />
            </div>
            <div>
              <h3 class="font-bold text-sm text-gray-900 dark:text-slate-100">
                {{ selectedPaths.length > 0 ? `Extract ${selectedPaths.length} Selected Items` : 'Extract Entire Archive' }}
              </h3>
              <span class="text-[11px] text-gray-400 dark:text-slate-400 font-mono truncate max-w-[220px] inline-block">
                {{ archiveName }}
              </span>
            </div>
          </div>
          <button
            type="button"
            @click="$emit('close')"
            class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-slate-200 rounded-lg cursor-pointer"
          >
            <FbIcon name="x" size="16px" />
          </button>
        </div>

        <!-- Extracting Progress State -->
        <div v-if="isExtracting" class="py-6 flex flex-col items-center space-y-3">
          <div class="w-8 h-8 rounded-full border-2 border-blue-500 border-t-transparent animate-spin"></div>
          <div class="text-center space-y-1">
            <p class="font-semibold text-xs text-gray-900 dark:text-white">Extracting files...</p>
            <p class="text-[11px] text-gray-400 dark:text-slate-400 font-mono">
              Please wait while entries are written to storage
            </p>
          </div>
        </div>

        <!-- Options Form -->
        <div v-else class="space-y-4 text-xs">
          <!-- Destination Input -->
          <div class="space-y-1.5">
            <label class="font-medium text-gray-700 dark:text-slate-300 text-[11px]">Destination Folder</label>
            <input
              v-model="destinationDir"
              type="text"
              class="w-full px-3 py-2 rounded-xl bg-gray-50 dark:bg-[#0c0e12] border border-gray-200 dark:border-white/[0.08] text-xs font-mono text-gray-800 dark:text-slate-200 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/20"
              placeholder="/destination/path"
            />
          </div>

          <!-- Create Subfolder Checkbox -->
          <label class="flex items-center space-x-2.5 cursor-pointer py-1">
            <input
              v-model="createSubfolder"
              type="checkbox"
              class="rounded border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-blue-500/30 cursor-pointer accent-blue-600"
            />
            <span class="text-gray-700 dark:text-slate-300 font-medium">
              Create subfolder <strong class="font-mono text-gray-900 dark:text-white">"{{ folderNameSuggestion }}"</strong>
            </span>
          </label>

          <!-- Conflict Handling Options -->
          <div class="space-y-2 pt-1">
            <span class="text-[11px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider block">
              If files already exist
            </span>
            <div class="grid grid-cols-3 gap-2 text-xs">
              <label
                :class="[
                  'px-3 py-2 rounded-xl border text-center cursor-pointer transition font-medium text-[11px]',
                  overwriteMode === 'overwrite'
                    ? 'border-blue-500 bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold'
                    : 'border-gray-200 dark:border-white/[0.08] text-gray-600 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-white/[0.03]'
                ]"
              >
                <input v-model="overwriteMode" type="radio" value="overwrite" class="sr-only" />
                <span>Replace</span>
              </label>

              <label
                :class="[
                  'px-3 py-2 rounded-xl border text-center cursor-pointer transition font-medium text-[11px]',
                  overwriteMode === 'keep_both'
                    ? 'border-blue-500 bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold'
                    : 'border-gray-200 dark:border-white/[0.08] text-gray-600 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-white/[0.03]'
                ]"
              >
                <input v-model="overwriteMode" type="radio" value="keep_both" class="sr-only" />
                <span>Keep Both</span>
              </label>

              <label
                :class="[
                  'px-3 py-2 rounded-xl border text-center cursor-pointer transition font-medium text-[11px]',
                  overwriteMode === 'skip'
                    ? 'border-blue-500 bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold'
                    : 'border-gray-200 dark:border-white/[0.08] text-gray-600 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-white/[0.03]'
                ]"
              >
                <input v-model="overwriteMode" type="radio" value="skip" class="sr-only" />
                <span>Skip</span>
              </label>
            </div>
          </div>

          <!-- Buttons -->
          <div class="flex items-center justify-end space-x-2.5 pt-3 border-t border-gray-100 dark:border-white/[0.06]">
            <button
              type="button"
              @click="$emit('close')"
              class="px-4 py-2 rounded-xl border border-gray-200 dark:border-white/[0.08] hover:bg-gray-100 dark:hover:bg-white/[0.05] text-gray-700 dark:text-slate-300 font-medium transition cursor-pointer text-xs"
            >
              Cancel
            </button>
            <button
              type="button"
              @click="submit"
              class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-semibold transition cursor-pointer text-xs shadow-sm active:scale-95"
            >
              Extract Now
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import FbIcon from '../../common/FbIcon.vue';
import type { ArchiveOverwriteMode } from '../../../api/archive';

const props = defineProps<{
  isOpen: boolean;
  archiveName: string;
  defaultDestination: string;
  selectedPaths: string[];
  isExtracting: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (
    e: 'extract',
    payload: {
      destinationDir: string;
      createSubfolder: boolean;
      overwriteMode: ArchiveOverwriteMode;
    }
  ): void;
}>();

const destinationDir = ref(props.defaultDestination || '/');
const createSubfolder = ref(true);
const overwriteMode = ref<ArchiveOverwriteMode>('overwrite');

watch(
  () => props.defaultDestination,
  (newDest) => {
    if (newDest) destinationDir.value = newDest;
  }
);

const folderNameSuggestion = computed(() => {
  return props.archiveName.replace(/\.(zip|tar\.gz|tgz|tar\.bz2|tar\.xz|tar|7z|rar)$/i, '');
});

function submit() {
  const raw = destinationDir.value.trim() || '/';
  const withLeading = raw.startsWith('/') ? raw : `/${raw}`;
  const clean = withLeading.length > 1 ? withLeading.replace(/\/+$/, '') : '/';
  emit('extract', {
    destinationDir: clean,
    createSubfolder: createSubfolder.value,
    overwriteMode: overwriteMode.value,
  });
}
</script>
