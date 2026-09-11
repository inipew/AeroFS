<template>
  <Transition name="ios-modal">
    <div
      v-if="uiStore.isUploadOpen"
      :class="[
        'fixed inset-0 z-50 bg-black/60 backdrop-blur-sm select-none font-sans text-xs',
        uiStore.isMobile ? 'flex flex-col justify-end p-0' : 'flex items-center justify-center p-4'
      ]"
      @click="handleClose"
    >
      <div
        :class="[
          'modal-card bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 shadow-2xl p-6',
          uiStore.isMobile ? 'w-full rounded-t-3xl rounded-b-none border-b-0 max-h-[85vh] pb-safe' : 'rounded-3xl max-w-md w-full'
        ]"
        @click.stop
      >
      <!-- Mobile Drag Indicator -->
      <div v-if="uiStore.isMobile" class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto -mt-2 mb-4"></div>

      <div class="flex items-center space-x-3 mb-4">
        <div class="w-10 h-10 rounded-2xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0">
          <FbIcon name="upload" size="20px" />
        </div>
        <div class="truncate flex-1">
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">Upload Files</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs font-mono truncate">
            Target: {{ currentTargetDirectory }}
          </p>
        </div>
        <button
          v-if="uiStore.isMobile"
          @click="handleClose"
          class="p-1 text-gray-400 hover:text-gray-700 dark:hover:text-white text-base"
        >
          ✕
        </button>
      </div>

      <!-- Mobile Quick Upload Options (Files, Camera, Photos) -->
      <div v-if="uiStore.isMobile" class="grid grid-cols-3 gap-2 mb-4">
        <button
          @click="fileInputRef?.click()"
          class="flex flex-col items-center justify-center p-3 rounded-2xl bg-gray-50 dark:bg-slate-800/80 border border-gray-200 dark:border-slate-700 hover:bg-gray-100 transition cursor-pointer"
        >
          <span class="text-xl mb-1">📁</span>
          <span class="font-semibold text-[11px] text-gray-800 dark:text-slate-200">Browse Files</span>
        </button>

        <button
          @click="cameraInputRef?.click()"
          class="flex flex-col items-center justify-center p-3 rounded-2xl bg-blue-50 dark:bg-blue-950/40 border border-blue-200 dark:border-blue-800/60 hover:bg-blue-100 transition cursor-pointer text-blue-600 dark:text-blue-400"
        >
          <span class="text-xl mb-1">📷</span>
          <span class="font-bold text-[11px]">Take Photo</span>
        </button>

        <button
          @click="mediaInputRef?.click()"
          class="flex flex-col items-center justify-center p-3 rounded-2xl bg-gray-50 dark:bg-slate-800/80 border border-gray-200 dark:border-slate-700 hover:bg-gray-100 transition cursor-pointer"
        >
          <span class="text-xl mb-1">🖼️</span>
          <span class="font-semibold text-[11px] text-gray-800 dark:text-slate-200">Gallery</span>
        </button>
      </div>

      <!-- Hidden Input Elements -->
      <input
        ref="fileInputRef"
        type="file"
        multiple
        class="hidden"
        @change="handleFileChange"
      />
      <input
        ref="cameraInputRef"
        type="file"
        accept="image/*,video/*"
        capture="environment"
        class="hidden"
        @change="handleFileChange"
      />
      <input
        ref="mediaInputRef"
        type="file"
        accept="image/*,video/*"
        multiple
        class="hidden"
        @change="handleFileChange"
      />

      <!-- Desktop Dropzone -->
      <div
        v-if="!uiStore.isMobile"
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
        <div class="flex justify-center mb-2 text-blue-600 dark:text-blue-400">
          <FbIcon name="upload" size="28px" />
        </div>
        <p class="text-gray-900 dark:text-slate-200 font-semibold mb-0.5">Click to select files or drag & drop</p>
        <p class="text-[11px] text-gray-500 dark:text-slate-500">Supports single and multiple file uploads</p>
      </div>

      <!-- Selected File List to Upload -->
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

      <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
        <button
          type="button"
          @click="handleClose"
          class="px-4 py-2.5 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
        >
          Cancel
        </button>
        <button
          type="button"
          :disabled="selectedFiles.length === 0"
          @click="startUpload"
          class="px-5 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
        >
          <span>Start Upload</span>
        </button>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useTransferStore } from '../../stores/transferStore';

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();
const transferStore = useTransferStore();

const fileInputRef = ref<HTMLInputElement | null>(null);
const cameraInputRef = ref<HTMLInputElement | null>(null);
const mediaInputRef = ref<HTMLInputElement | null>(null);

const isDragging = ref(false);
const selectedFiles = ref<File[]>([]);

const currentTargetDirectory = computed(() => {
  const p = workspaceStore.getPanel(workspaceStore.activePanelId);
  return p.path === '/' ? '/' : p.path;
});

function handleClose() {
  uiStore.isUploadOpen = false;
  selectedFiles.value = [];
}

function handleFileChange(e: Event) {
  const target = e.target as HTMLInputElement;
  if (target.files && target.files.length > 0) {
    selectedFiles.value = Array.from(target.files);
  }
}

function handleDrop(e: DragEvent) {
  isDragging.value = false;
  if (e.dataTransfer && e.dataTransfer.files.length > 0) {
    selectedFiles.value = Array.from(e.dataTransfer.files);
  }
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function startUpload() {
  if (selectedFiles.value.length === 0) return;

  const filesToUpload = [...selectedFiles.value];
  const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
  const connId = activeP.connectionId || 'local';
  const targetFolder = activeP.path;
  const existingFileNames = activeP.entries.map((e) => e.name);

  // Close dialog and reset selection immediately
  handleClose();

  // Hand off to transfer store
  void transferStore.submitUploadBatch({
    connectionId: connId,
    targetDir: targetFolder,
    files: filesToUpload,
    existingNames: existingFileNames,
  });
}
</script>
