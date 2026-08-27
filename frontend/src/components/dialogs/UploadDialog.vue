<template>
  <div
    v-if="uiStore.isUploadOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs animate-in fade-in duration-150"
  >
    <div class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl">
      <div class="flex items-center space-x-3 mb-3">
        <div class="w-10 h-10 rounded-2xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon name="upload" size="20px" />
        </div>
        <div>
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">Upload Files</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs font-mono truncate max-w-[250px]">
            To {{ currentTargetDirectory }}
          </p>
        </div>
      </div>

      <!-- Dropzone -->
      <div
        @dragover.prevent="isDragging = true"
        @dragleave.prevent="isDragging = false"
        @drop.prevent="handleDrop"
        :class="[
          'border-2 border-dashed rounded-2xl p-6 text-center transition cursor-pointer mb-4',
          isDragging
            ? 'border-blue-500 bg-blue-500/10'
            : 'border-gray-300 dark:border-slate-700 hover:border-gray-400 dark:hover:border-slate-600 bg-gray-50 dark:bg-slate-950/60'
        ]"
        @click="fileInputRef?.click()"
      >
        <input
          ref="fileInputRef"
          type="file"
          multiple
          class="hidden"
          @change="handleFileChange"
        />
        <div class="flex justify-center mb-2 text-blue-600 dark:text-blue-400">
          <FbIcon name="upload" size="28px" />
        </div>
        <p class="text-gray-900 dark:text-slate-200 font-semibold mb-0.5">Click to select files or drag & drop</p>
        <p class="text-[11px] text-gray-500 dark:text-slate-500">Supports single and multiple file uploads</p>
      </div>

      <!-- File List to Upload -->
      <div v-if="selectedFiles.length > 0" class="mb-4">
        <span class="text-[11px] text-gray-700 dark:text-slate-400 font-semibold mb-1 block">
          Selected ({{ selectedFiles.length }}):
        </span>
        <div class="max-h-28 overflow-y-auto space-y-1 bg-gray-50 dark:bg-slate-950 p-2 rounded-xl border border-gray-200 dark:border-slate-800 text-[11px]">
          <div
            v-for="(f, i) in selectedFiles"
            :key="i"
            class="flex items-center justify-between text-gray-800 dark:text-slate-300"
          >
            <span class="truncate max-w-[240px]">{{ f.name }}</span>
            <span class="text-gray-400 dark:text-slate-500 font-mono">{{ formatBytes(f.size) }}</span>
          </div>
        </div>
      </div>

      <!-- Progress Bar -->
      <div v-if="uploading" class="mb-4 space-y-1.5">
        <div class="flex justify-between text-[11px] text-gray-600 dark:text-slate-400">
          <span>Uploading...</span>
          <span>{{ progress }}%</span>
        </div>
        <div class="w-full bg-gray-200 dark:bg-slate-800 rounded-full h-1.5 overflow-hidden">
          <div
            class="bg-blue-600 h-1.5 transition-all duration-200"
            :style="{ width: `${progress}%` }"
          ></div>
        </div>
      </div>

      <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
        <button
          type="button"
          :disabled="uploading"
          @click="uiStore.isUploadOpen = false"
          class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="button"
          :disabled="uploading || selectedFiles.length === 0"
          @click="startUpload"
          class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
        >
          <span v-if="uploading" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
          <span>{{ uploading ? 'Uploading...' : 'Start Upload' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { uploadFileApi } from '../../api/files';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useUiStore } from '../../stores/uiStore';

const workspaceStore = useWorkspaceStore();
const uiStore = useUiStore();

const fileInputRef = ref<HTMLInputElement | null>(null);
const isDragging = ref(false);
const selectedFiles = ref<File[]>([]);
const uploading = ref(false);
const progress = ref(0);

const currentTargetDirectory = computed(() => {
  const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
  return activeP.path || '/';
});

watch(
  () => uiStore.isUploadOpen,
  (open) => {
    if (open) {
      selectedFiles.value = [];
      progress.value = 0;
      uploading.value = false;
    }
  }
);

function handleFileChange(e: Event) {
  const target = e.target as HTMLInputElement;
  if (target.files) {
    selectedFiles.value = Array.from(target.files);
  }
}

function handleDrop(e: DragEvent) {
  isDragging.value = false;
  if (e.dataTransfer?.files) {
    selectedFiles.value = Array.from(e.dataTransfer.files);
  }
}

async function startUpload() {
  if (selectedFiles.value.length === 0) return;
  uploading.value = true;
  progress.value = 0;

  try {
    const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
    const connId = activeP.connectionId;
    const destPath = activeP.path;

    for (let i = 0; i < selectedFiles.value.length; i++) {
      await uploadFileApi(connId, destPath, selectedFiles.value[i], (p) => {
        progress.value = Math.round(((i + p / 100) / selectedFiles.value.length) * 100);
      });
    }

    uiStore.showToast(`Uploaded ${selectedFiles.value.length} file(s)`, 'success');
    uiStore.isUploadOpen = false;

    // Immediately reload entries!
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Upload failed', 'error');
  } finally {
    uploading.value = false;
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
</script>
