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
            <div class="w-9 h-9 rounded-xl bg-blue-500/10 text-blue-500 flex items-center justify-center shrink-0">
              <FbIcon name="clock" size="18px" />
            </div>
            <div>
              <h3 class="text-sm font-bold text-gray-900 dark:text-white">Recent Files</h3>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Quickly access recently viewed files and directories.</p>
            </div>
          </div>

          <div class="flex items-center space-x-2">
            <button
              v-if="recentStore.recentItems.length > 0"
              @click="recentStore.clearRecent()"
              class="px-2.5 py-1 text-[11px] text-gray-500 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-950/40 rounded-xl transition cursor-pointer font-medium"
              title="Clear all recent history"
            >
              Clear All
            </button>
            <button
              @click="isOpen = false"
              class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            >
              ✕
            </button>
          </div>
        </div>

        <!-- Content -->
        <div class="p-6 overflow-y-auto flex-1 space-y-4">
          <!-- Sort Active Panel Helper -->
          <div class="p-3.5 bg-blue-50/60 dark:bg-blue-950/20 border border-blue-200/60 dark:border-blue-900/40 rounded-2xl flex items-center justify-between text-xs text-blue-900 dark:text-blue-200">
            <div class="flex items-center space-x-2.5 truncate mr-2">
              <FbIcon name="sort" size="15px" class="text-blue-500 shrink-0" />
              <span class="truncate">Want to see the newest files in your current folder?</span>
            </div>
            <button
              @click="handleSortCurrentFolder"
              class="px-2.5 py-1 bg-blue-600 hover:bg-blue-700 text-white font-bold rounded-xl text-[11px] shrink-0 cursor-pointer shadow-xs transition active:scale-95"
            >
              Sort Folder by Date
            </button>
          </div>

          <!-- Empty State -->
          <div v-if="recentStore.recentItems.length === 0" class="py-16 text-center text-gray-400 dark:text-slate-500 space-y-2">
            <div class="w-12 h-12 rounded-2xl bg-gray-100 dark:bg-slate-900 mx-auto flex items-center justify-center text-blue-400">
              <FbIcon name="clock" size="24px" />
            </div>
            <p class="font-bold text-gray-700 dark:text-slate-300">No recent files yet</p>
            <p class="text-[11px]">Files and folders you open, edit, or preview will appear here.</p>
          </div>

          <!-- Recent List -->
          <div v-else class="space-y-2">
            <div
              v-for="item in recentStore.recentItems"
              :key="item.entry.path"
              class="p-3 sm:p-3.5 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 flex items-center justify-between gap-3 hover:border-blue-500/30 transition cursor-pointer group"
              @click="handleOpenItem(item)"
            >
              <div class="flex items-center space-x-3 truncate">
                <div class="w-8 h-8 rounded-xl bg-blue-500/10 text-blue-500 flex items-center justify-center shrink-0 group-hover:scale-105 transition-transform">
                  <FbIcon :name="getItemIcon(item.entry)" size="16px" />
                </div>
                <div class="truncate space-y-0.5">
                  <p class="font-bold text-gray-900 dark:text-white text-xs truncate">{{ item.entry.name }}</p>
                  <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ item.entry.path }} ({{ item.connectionId }})</p>
                </div>
              </div>

              <div class="flex items-center space-x-2 shrink-0">
                <span class="text-[10px] text-gray-400 dark:text-slate-500 font-mono">{{ formatTimeAgo(item.accessedAt) }}</span>
                <button
                  @click.stop="recentStore.removeRecent(item.connectionId, item.entry.path)"
                  class="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-950/40 rounded-lg transition opacity-0 group-hover:opacity-100 cursor-pointer"
                  title="Remove from history"
                >
                  ✕
                </button>
              </div>
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
import { useRecentStore } from '../../stores/recentStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import { readFileApi, getDownloadUrl } from '../../api/files';
import type { FileEntry } from '../../types/vfs';
import type { IconName } from '../../utils/icons';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const recentStore = useRecentStore();
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

function getItemIcon(entry: FileEntry): IconName {
  if (entry.kind === 'directory') return 'folder';
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv', 'avi'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'flac', 'm4a', 'ogg'].includes(ext)) return 'audio';
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext)) return 'archive';
  if (['pdf'].includes(ext)) return 'pdf';
  if (['txt', 'md', 'rs', 'ts', 'js', 'json', 'toml', 'yaml', 'yml', 'html', 'css', 'py', 'sh', 'sql'].includes(ext)) return 'code';
  return 'file';
}

function formatTimeAgo(isoDate: string): string {
  if (!isoDate) return '';
  const now = Date.now();
  const past = new Date(isoDate).getTime();
  const diffSec = Math.floor((now - past) / 1000);
  if (diffSec < 60) return 'Just now';
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  const diffDays = Math.floor(diffHours / 24);
  return `${diffDays}d ago`;
}

function handleSortCurrentFolder() {
  isOpen.value = false;
  const panel = workspaceStore.getPanel(workspaceStore.activePanelId);
  panel.view.sortField = 'modified';
  panel.view.sortOrder = 'desc';
  workspaceStore.saveState();
  workspaceStore.refreshPanel(workspaceStore.activePanelId);
  uiStore.showToast('Sorted current folder by most recently modified', 'info');
}

async function handleOpenItem(item: any) {
  isOpen.value = false;
  fileStore.currentConnectionId = item.connectionId;

  if (item.entry.kind === 'directory') {
    workspaceStore.navigatePanel(workspaceStore.activePanelId, item.entry.path);
  } else {
    const ext = item.entry.name.split('.').pop()?.toLowerCase() || '';
    const textExts = ['txt', 'md', 'rs', 'ts', 'js', 'json', 'toml', 'yaml', 'yml', 'html', 'css', 'py', 'sh', 'sql', 'toml', 'env'];

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
        getDownloadUrl(item.connectionId, item.entry.path),
        item.entry,
        [item.entry],
        item.connectionId
      );
    }
  }
}
</script>
