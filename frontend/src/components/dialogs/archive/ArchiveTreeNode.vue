<template>
  <div class="select-none text-xs">
    <!-- Folder Row -->
    <div
      @click="handleClick"
      :class="[
        'flex items-center space-x-1.5 px-2 py-1.5 rounded-lg cursor-pointer transition duration-150 group',
        currentSubpath === folder.path
          ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold border-l-2 border-blue-500 pl-1.5'
          : 'text-gray-700 dark:text-slate-300 hover:bg-gray-200/50 dark:hover:bg-white/[0.04]'
      ]"
      :style="{ paddingLeft: depth > 0 ? `${depth * 12 + 8}px` : undefined }"
    >
      <!-- Expand / Collapse Toggle Button -->
      <button
        type="button"
        @click.stop="toggle"
        class="w-4 h-4 flex items-center justify-center rounded text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 shrink-0"
      >
        <FbIcon
          :name="isExpanded ? 'chevron-down' : 'chevron-right'"
          size="11px"
        />
      </button>

      <FbIcon name="folder" size="14px" class="text-sky-500 dark:text-sky-400 shrink-0" />
      <span class="truncate text-xs font-medium">{{ folder.name }}</span>
    </div>

    <!-- Subfolders when expanded -->
    <div v-if="isExpanded" class="space-y-0.5 mt-0.5">
      <!-- Loading indicator -->
      <div
        v-if="isLoading"
        class="text-[11px] text-gray-400 flex items-center space-x-1.5 py-1"
        :style="{ paddingLeft: `${(depth + 1) * 12 + 16}px` }"
      >
        <div class="w-3 h-3 border border-blue-500 border-t-transparent rounded-full animate-spin"></div>
        <span>Loading...</span>
      </div>

      <!-- Recursive Child Nodes -->
      <template v-else-if="subfolders && subfolders.length > 0">
        <ArchiveTreeNode
          v-for="sub in subfolders"
          :key="sub.path"
          :folder="sub"
          :connection-id="connectionId"
          :archive-path="archivePath"
          :current-subpath="currentSubpath"
          :depth="depth + 1"
          @navigate="$emit('navigate', $event)"
        />
      </template>

      <!-- Empty subfolder indicator -->
      <div
        v-else
        class="text-[10px] text-gray-400 dark:text-slate-600 italic py-0.5"
        :style="{ paddingLeft: `${(depth + 1) * 12 + 16}px` }"
      >
        No subfolders
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import FbIcon from '../../common/FbIcon.vue';
import type { VirtualArchiveEntry } from '../../../api/archive';
import { listArchiveEntriesApi } from '../../../api/archive';

const props = withDefaults(
  defineProps<{
    folder: VirtualArchiveEntry;
    connectionId: string;
    archivePath: string;
    currentSubpath: string;
    depth?: number;
  }>(),
  {
    depth: 0,
  }
);

const emit = defineEmits<{
  (e: 'navigate', subpath: string): void;
}>();

const isExpanded = ref(false);
const isLoading = ref(false);
const subfolders = ref<VirtualArchiveEntry[] | null>(null);

async function toggle() {
  if (isExpanded.value) {
    isExpanded.value = false;
    return;
  }

  isExpanded.value = true;
  if (subfolders.value === null) {
    isLoading.value = true;
    try {
      const items = await listArchiveEntriesApi(
        props.connectionId,
        props.archivePath,
        props.folder.path
      );
      subfolders.value = items.filter((i) => i.kind === 'directory');
    } catch {
      subfolders.value = [];
    } finally {
      isLoading.value = false;
    }
  }
}

function handleClick() {
  emit('navigate', props.folder.path);
  if (!isExpanded.value) {
    void toggle();
  }
}
</script>
