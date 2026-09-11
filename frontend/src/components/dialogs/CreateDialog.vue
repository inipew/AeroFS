<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs"
      @click="closeDialog"
    >
      <div class="modal-card bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-sm w-full p-6 shadow-2xl" @click.stop>
      <div class="flex items-center space-x-3 mb-3">
        <div class="w-10 h-10 rounded-2xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon :name="currentCreateType === 'file' ? 'file' : 'folder'" size="20px" />
        </div>
        <div>
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">
            Create New {{ currentCreateType === 'file' ? 'File' : 'Folder' }}
          </h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs font-mono truncate max-w-[200px]">
            In {{ currentTargetDirectory }}
          </p>
        </div>
      </div>

      <form @submit.prevent="handleSubmit" class="space-y-4">
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Name</label>
          <input
            ref="inputRef"
            v-model="name"
            type="text"
            :placeholder="currentCreateType === 'file' ? 'e.g. index.html' : 'e.g. new_folder'"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner font-medium"
            required
          />
        </div>

        <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
          <button
            type="button"
            @click="closeDialog"
            class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="submit"
            :disabled="loading || !name.trim()"
            class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
          >
            <span v-if="loading" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>{{ loading ? 'Creating...' : 'Create' }}</span>
          </button>
        </div>
      </form>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { createFileApi, createDirectoryApi } from '../../api/files';
import { normalizeApiError } from '../../utils/errorNormalizer';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useUiStore } from '../../stores/uiStore';
import { useOverlayStore } from '../../overlays/overlayStore';
import { useHistoryStore } from '../../stores/historyStore';
import type { PanelId } from '../../types/workspace';

const workspaceStore = useWorkspaceStore();
const uiStore = useUiStore();
const overlayStore = useOverlayStore();
const historyStore = useHistoryStore();

const name = ref('');
const loading = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);

const isOpen = computed(() => uiStore.isCreateOpen || overlayStore.current?.type === 'create');

const currentPanelId = computed<PanelId>(() => {
  if (overlayStore.current?.type === 'create') {
    return overlayStore.current.panelId;
  }
  return workspaceStore.activePanelId;
});

const currentCreateType = computed<'file' | 'directory'>(() => {
  if (overlayStore.current?.type === 'create' && overlayStore.current.initialType) {
    return overlayStore.current.initialType;
  }
  return uiStore.createType;
});

const currentPanel = computed(() => workspaceStore.getPanel(currentPanelId.value));

const currentTargetDirectory = computed(() => {
  return currentPanel.value?.location?.path || '/';
});

function closeDialog() {
  uiStore.isCreateOpen = false;
  if (overlayStore.current?.type === 'create') {
    overlayStore.close();
  }
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isOpen.value) {
    closeDialog();
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});

watch(
  isOpen,
  (open) => {
    if (open) {
      name.value = '';
      nextTick(() => inputRef.value?.focus());
    }
  },
  { immediate: true }
);

async function handleSubmit() {
  const trimmed = name.value.trim();
  if (!trimmed) return;
  loading.value = true;
  try {
    const p = currentPanel.value;
    const basePath = p.location.path;
    const fullPath = basePath === '/' ? `/${trimmed}` : `${basePath}/${trimmed}`;
    const connId = p.location.connectionId;
    const kind = currentCreateType.value;

    if (kind === 'file') {
      await createFileApi(connId, fullPath);
    } else {
      await createDirectoryApi(connId, fullPath);
    }

    historyStore.pushOperation({
      type: 'create',
      description: `Created ${kind} ${trimmed}`,
      connectionId: connId,
      path: fullPath,
      kind,
    });

    uiStore.showToast(`Created ${trimmed}`, 'success');
    closeDialog();

    // Refresh panel
    await workspaceStore.refreshPanel(currentPanelId.value);
  } catch (err: unknown) {
    uiStore.showToast(normalizeApiError(err).message || 'Creation failed', 'error');
  } finally {
    loading.value = false;
  }
}
</script>
