<template>
  <aside class="flex flex-col h-full select-none text-xs border-r border-gray-200/80 dark:border-white/[0.08] bg-gray-50/50 dark:bg-[#0c0e12]/60 overflow-hidden">
    <!-- Header -->
    <div class="px-3.5 py-2.5 flex items-center justify-between border-b border-gray-200/70 dark:border-white/[0.06] shrink-0 text-[11px] font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider">
      <span>Folders</span>
      <button
        type="button"
        @click="$emit('collapse')"
        class="p-1 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-200/60 dark:hover:bg-white/[0.08] transition cursor-pointer"
        title="Hide Folder Tree"
      >
        <FbIcon name="panel-left" size="13px" />
      </button>
    </div>

    <!-- Tree Content -->
    <div class="flex-1 overflow-y-auto overflow-x-hidden p-2 space-y-0.5 scrollbar-thin">
      <!-- Root Item -->
      <div
        @click="$emit('navigate', '')"
        :class="[
          'flex items-center space-x-2 px-2 py-1.5 rounded-lg cursor-pointer transition duration-150',
          currentSubpath === ''
            ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold border-l-2 border-blue-500 pl-1.5'
            : 'text-gray-700 dark:text-slate-300 hover:bg-gray-200/50 dark:hover:bg-white/[0.04]'
        ]"
      >
        <FbIcon name="archive" size="14px" class="text-amber-500 dark:text-amber-400 shrink-0" />
        <span class="truncate font-medium flex-1">Root Archive</span>
      </div>

      <!-- Recursive Tree Nodes -->
      <ArchiveTreeNode
        v-for="folder in rootFolders"
        :key="folder.path"
        :folder="folder"
        :connection-id="connectionId"
        :archive-path="archivePath"
        :current-subpath="currentSubpath"
        :depth="0"
        @navigate="$emit('navigate', $event)"
      />
    </div>
  </aside>
</template>

<script setup lang="ts">
import FbIcon from '../../common/FbIcon.vue';
import type { VirtualArchiveEntry } from '../../../api/archive';
import ArchiveTreeNode from './ArchiveTreeNode.vue';

defineProps<{
  connectionId: string;
  archivePath: string;
  currentSubpath: string;
  rootFolders: VirtualArchiveEntry[];
}>();

defineEmits<{
  (e: 'navigate', subpath: string): void;
  (e: 'collapse'): void;
}>();
</script>
