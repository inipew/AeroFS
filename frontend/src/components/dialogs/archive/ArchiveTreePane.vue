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
      <div v-for="folder in rootFolders" :key="folder.path">
        <div
          @click="handleFolderClick(folder)"
          :class="[
            'flex items-center space-x-1.5 px-2 py-1.5 rounded-lg cursor-pointer transition duration-150 group',
            currentSubpath === folder.path
              ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold border-l-2 border-blue-500 pl-1.5'
              : 'text-gray-700 dark:text-slate-300 hover:bg-gray-200/50 dark:hover:bg-white/[0.04]'
          ]"
        >
          <!-- Expand/Collapse toggle button -->
          <button
            type="button"
            @click.stop="toggleFolder(folder)"
            class="w-4 h-4 flex items-center justify-center rounded text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 shrink-0"
          >
            <FbIcon
              :name="expandedFolders.has(folder.path) ? 'chevron-down' : 'chevron-right'"
              size="11px"
            />
          </button>

          <FbIcon name="folder" size="14px" class="text-sky-500 dark:text-sky-400 shrink-0" />
          <span class="truncate text-xs">{{ folder.name }}</span>
        </div>

        <!-- Subfolders when expanded -->
        <div
          v-if="expandedFolders.has(folder.path)"
          class="pl-4 border-l border-gray-200 dark:border-white/[0.06] ml-3 mt-0.5 space-y-0.5"
        >
          <div
            v-if="loadingFolders.has(folder.path)"
            class="px-2 py-1 text-[11px] text-gray-400 flex items-center space-x-1.5"
          >
            <div class="w-3 h-3 border border-blue-500 border-t-transparent rounded-full animate-spin"></div>
            <span>Loading...</span>
          </div>

          <template v-else-if="subfolderMap.get(folder.path)?.length">
            <div
              v-for="sub in subfolderMap.get(folder.path)"
              :key="sub.path"
              @click="$emit('navigate', sub.path)"
              :class="[
                'flex items-center space-x-1.5 px-2 py-1 rounded-lg cursor-pointer transition duration-150',
                currentSubpath === sub.path
                  ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400 font-bold border-l-2 border-blue-500 pl-1.5'
                  : 'text-gray-600 dark:text-slate-400 hover:bg-gray-200/50 dark:hover:bg-white/[0.04] hover:text-gray-900 dark:hover:text-slate-200'
              ]"
            >
              <FbIcon name="folder" size="13px" class="text-sky-500/80 dark:text-sky-400/80 shrink-0" />
              <span class="truncate text-[11px]">{{ sub.name }}</span>
            </div>
          </template>

          <div
            v-else
            class="px-2 py-1 text-[10px] text-gray-400 dark:text-slate-600 italic"
          >
            No subfolders
          </div>
        </div>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import FbIcon from '../../common/FbIcon.vue';
import type { VirtualArchiveEntry } from '../../../api/archive';
import { listArchiveEntriesApi } from '../../../api/archive';

const props = defineProps<{
  connectionId: string;
  archivePath: string;
  currentSubpath: string;
  rootFolders: VirtualArchiveEntry[];
}>();

const emit = defineEmits<{
  (e: 'navigate', subpath: string): void;
  (e: 'collapse'): void;
}>();

const expandedFolders = ref<Set<string>>(new Set());
const loadingFolders = ref<Set<string>>(new Set());
const subfolderMap = ref<Map<string, VirtualArchiveEntry[]>>(new Map());

async function toggleFolder(folder: VirtualArchiveEntry) {
  if (expandedFolders.value.has(folder.path)) {
    expandedFolders.value.delete(folder.path);
    return;
  }

  expandedFolders.value.add(folder.path);

  if (!subfolderMap.value.has(folder.path)) {
    loadingFolders.value.add(folder.path);
    try {
      const items = await listArchiveEntriesApi(
        props.connectionId,
        props.archivePath,
        folder.path
      );
      subfolderMap.value.set(
        folder.path,
        items.filter((i) => i.kind === 'directory')
      );
    } catch {
      subfolderMap.value.set(folder.path, []);
    } finally {
      loadingFolders.value.delete(folder.path);
    }
  }
}

function handleFolderClick(folder: VirtualArchiveEntry) {
  emit('navigate', folder.path);
  if (!expandedFolders.value.has(folder.path)) {
    void toggleFolder(folder);
  }
}
</script>
