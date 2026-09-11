<template>
  <div
    class="flex-1 flex flex-col items-center justify-center p-8 text-center select-none"
    :class="[isDragOver ? 'bg-blue-50/50 dark:bg-blue-950/30' : '']"
  >
    <div class="w-16 h-16 rounded-3xl bg-gray-100 dark:bg-slate-800/80 flex items-center justify-center text-gray-400 dark:text-slate-500 mb-3 shadow-2xs">
      <FbIcon name="folder" size="28px" />
    </div>

    <h3 class="text-sm font-semibold text-gray-800 dark:text-slate-200">
      {{ isDragOver ? 'Drop files here' : 'Folder is empty' }}
    </h3>
    <p class="text-xs text-gray-500 dark:text-slate-400 mt-1 max-w-xs">
      {{ isDragOver ? 'Release to upload directly into this folder' : 'Drag files here or create new items using the toolbar.' }}
    </p>

    <div v-if="!isDragOver" class="flex items-center space-x-2 mt-4">
      <button
        type="button"
        @click="$emit('upload')"
        class="px-3 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs shadow-xs transition cursor-pointer active:scale-95 duration-fast ease-spring flex items-center space-x-1.5"
      >
        <FbIcon name="upload" size="13px" />
        <span>Upload Files</span>
      </button>

      <button
        type="button"
        @click="$emit('newFolder')"
        class="px-3 py-1.5 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-gray-700 dark:text-slate-300 font-medium text-xs shadow-2xs transition cursor-pointer active:scale-95 duration-fast ease-spring flex items-center space-x-1.5"
      >
        <FbIcon name="new-folder" size="13px" class="text-amber-500" />
        <span>New Folder</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import FbIcon from '../../../components/common/FbIcon.vue';

withDefaults(
  defineProps<{
    isDragOver?: boolean;
  }>(),
  {
    isDragOver: false,
  }
);

defineEmits<{
  (e: 'upload'): void;
  (e: 'newFolder'): void;
}>();
</script>
