<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen && connection"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs"
      @click="isOpen = false"
    >
      <div
        class="modal-card bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl relative space-y-4 animate-in fade-in zoom-in-95 duration-200"
        @click.stop
      >
        <!-- Header -->
        <div class="flex items-center space-x-3">
          <div class="w-10 h-10 rounded-2xl bg-rose-500/10 dark:bg-rose-500/20 text-rose-600 dark:text-rose-400 flex items-center justify-center shrink-0">
            <FbIcon name="trash" size="20px" />
          </div>
          <div>
            <h3 class="text-base font-bold text-gray-900 dark:text-white">Delete Storage Source</h3>
            <p class="text-gray-500 dark:text-slate-400 text-xs">Remove storage connection from AeroFS</p>
          </div>
        </div>

        <!-- Connection Details Preview Box -->
        <div class="p-3 bg-gray-50 dark:bg-slate-950/60 border border-gray-200/80 dark:border-slate-800 rounded-2xl flex items-center justify-between">
          <div class="min-w-0 flex-1 pr-2">
            <p class="font-bold text-gray-900 dark:text-white truncate text-sm">
              {{ connection.name }}
            </p>
            <p class="text-xs text-gray-500 dark:text-slate-400 font-mono truncate">
              {{ connection.host ? `${connection.host}:${connection.port ?? ''}` : connection.base_path }}
            </p>
          </div>
          <span class="text-[10px] font-mono uppercase px-2 py-0.5 rounded-md bg-gray-200/80 dark:bg-slate-800 text-gray-700 dark:text-slate-300 font-semibold shrink-0">
            {{ connection.provider }}
          </span>
        </div>

        <!-- Warning Callout -->
        <div class="p-3 bg-amber-50 dark:bg-amber-950/30 border border-amber-200/60 dark:border-amber-800/40 rounded-2xl text-amber-800 dark:text-amber-300 text-[11px] leading-relaxed">
          <strong>Notice:</strong> This action will disconnect the storage from AeroFS and abort any ongoing transfers for this connection. Files on the remote storage server itself will <strong>not</strong> be deleted.
        </div>

        <!-- Action Buttons -->
        <div class="flex justify-between items-center pt-2 border-t border-gray-100 dark:border-slate-800">
          <button
            type="button"
            @click="isOpen = false"
            class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
          >
            Cancel
          </button>

          <button
            type="button"
            @click="handleDelete"
            :disabled="deleting"
            class="px-5 py-2 rounded-xl bg-rose-600 hover:bg-rose-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
          >
            <span v-if="deleting" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>{{ deleting ? 'Deleting...' : 'Delete Connection' }}</span>
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { deleteConnectionApi, listConnectionsApi } from '../../api/connections';
import { queryClient } from '../../queryClient';
import { queryKeys } from '../../api/queryKeys';
import { normalizeApiError } from '../../utils/errorNormalizer';
import { useConnectionStore } from '../../stores/connectionStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import type { Connection } from '../../types/connection';

const props = defineProps<{
  modelValue: boolean;
  connection: Connection | null;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'deleted', connId: string): void;
}>();

const connStore = useConnectionStore();
const workspaceStore = useWorkspaceStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const isOpen = ref(props.modelValue);
const deleting = ref(false);

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

async function handleDelete() {
  if (!props.connection || deleting.value) return;
  const connId = props.connection.id;
  deleting.value = true;

  try {
    const res = await deleteConnectionApi(connId);
    uiStore.showToast(res.message || 'Storage connection deleted', 'success');

    // If active in left or right panel, fallback immediately to local root '/'
    if (workspaceStore.leftPanel.connectionId === connId) {
      await workspaceStore.switchPanelConnection('left', 'local', '/');
    }
    if (workspaceStore.rightPanel.connectionId === connId) {
      await workspaceStore.switchPanelConnection('right', 'local', '/');
    }
    if (fileStore.currentConnectionId === connId) {
      fileStore.currentConnectionId = 'local';
    }

    // Invalidate TanStack query cache
    queryClient.invalidateQueries({ queryKey: queryKeys.connections() });
    queryClient.invalidateQueries({ queryKey: queryKeys.connection(connId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.directories() });

    // Refresh pinia store
    const conns = await listConnectionsApi();
    connStore.connections = conns;

    emit('deleted', connId);
    isOpen.value = false;
  } catch (err: unknown) {
    uiStore.showToast(normalizeApiError(err).message, 'error');
  } finally {
    deleting.value = false;
  }
}
</script>
