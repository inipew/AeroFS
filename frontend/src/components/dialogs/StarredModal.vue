<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs"
      @click="isOpen = false"
    >
      <div
        class="modal-card bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-2xl w-full flex flex-col shadow-2xl overflow-hidden max-h-[85vh]"
        @click.stop
      >
      <!-- Header -->
      <div class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-amber-500/10 text-amber-500 flex items-center justify-center shrink-0">
            <FbIcon name="star" size="18px" />
          </div>
          <div>
            <h3 class="text-sm font-bold text-gray-900 dark:text-white">Starred Bookmarks</h3>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">Quick access to bookmarked files and directories.</p>
          </div>
        </div>

        <button
          @click="isOpen = false"
          class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        >
          ✕
        </button>
      </div>

      <!-- Content -->
      <div class="p-6 overflow-y-auto flex-1 space-y-4">
        <!-- Empty State -->
        <div v-if="starredStore.starredItems.length === 0" class="py-16 text-center text-gray-400 dark:text-slate-500 space-y-2">
          <div class="w-12 h-12 rounded-2xl bg-gray-100 dark:bg-slate-900 mx-auto flex items-center justify-center text-amber-400">
            <FbIcon name="star" size="24px" />
          </div>
          <p class="font-bold text-gray-700 dark:text-slate-300">No starred bookmarks yet</p>
          <p class="text-[11px]">Right-click any file/folder in the explorer and select "⭐ Add to Starred".</p>
        </div>

        <!-- Starred List -->
        <div v-else class="space-y-2.5">
          <div
            v-for="item in starredStore.starredItems"
            :key="item.entry.path"
            class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 flex items-center justify-between gap-4 hover:border-blue-500/30 transition cursor-pointer"
            @click="handleOpenItem(item)"
          >
            <div class="flex items-center space-x-3 truncate">
              <div class="w-8 h-8 rounded-xl bg-amber-500/10 text-amber-500 flex items-center justify-center shrink-0">
                <FbIcon :name="item.entry.kind === 'directory' ? 'folder' : 'file'" size="16px" />
              </div>
              <div class="truncate space-y-0.5">
                <p class="font-bold text-gray-900 dark:text-white text-xs truncate">{{ item.entry.name }}</p>
                <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ item.entry.path }} ({{ item.connectionId }})</p>
              </div>
            </div>

            <!-- Remove Button -->
            <button
              @click.stop="starredStore.toggleStar(item.connectionId, item.entry)"
              class="p-2 text-amber-500 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-950/40 rounded-xl transition cursor-pointer"
              title="Remove bookmark"
            >
              ★
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useStarredStore } from '../../stores/starredStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import { readFileApi } from '../../api/files';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const starredStore = useStarredStore();
const workspaceStore = useWorkspaceStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const isOpen = ref(props.modelValue);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function handleOpenItem(item: any) {
  isOpen.value = false;
  fileStore.currentConnectionId = item.connectionId;

  if (item.entry.kind === 'directory') {
    workspaceStore.navigatePanel(workspaceStore.activePanelId, item.entry.path);
  } else {
    // Open in editor if text or viewer if media
    const ext = item.entry.name.split('.').pop()?.toLowerCase() || '';
    const textExts = ['txt', 'md', 'rs', 'ts', 'js', 'json', 'toml', 'yaml', 'yml', 'html', 'css', 'py', 'sh'];

    if (textExts.includes(ext)) {
      try {
        const resp = await readFileApi(item.connectionId, item.entry.path);
        uiStore.openEditor(item.entry, resp.content, resp.etag, item.connectionId);
      } catch (err: any) {
        uiStore.showToast('Failed to load file', 'error');
      }
    } else {
      uiStore.openMediaViewer(
        item.entry.name,
        `/api/v1/connections/${item.connectionId}/files/content?path=${encodeURIComponent(item.entry.path)}`,
        item.entry,
        [item.entry],
        item.connectionId
      );
    }
  }
}
</script>
