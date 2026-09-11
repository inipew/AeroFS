<template>
  <aside class="flex flex-col h-full select-none text-xs border-l border-gray-200/80 dark:border-white/[0.08] bg-gray-50/40 dark:bg-[#0c0e12]/60 overflow-hidden">
    <!-- Header -->
    <div class="px-4 py-2.5 flex items-center justify-between border-b border-gray-200/70 dark:border-white/[0.06] shrink-0 text-[11px]">
      <div class="flex items-center space-x-2 truncate">
        <span class="font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider">Inspector</span>
        <span v-if="entry" class="px-1.5 py-0.2 rounded text-[9px] font-mono font-bold uppercase bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20 truncate">
          {{ getFileExt(entry.name) }}
        </span>
      </div>
      <button
        type="button"
        @click="$emit('collapse')"
        class="p-1 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-200/60 dark:hover:bg-white/[0.08] transition cursor-pointer"
        title="Hide Inspector"
      >
        <FbIcon name="x" size="13px" />
      </button>
    </div>

    <!-- Empty / No Selection State -->
    <div
      v-if="!entry"
      class="flex-1 flex flex-col items-center justify-center p-6 text-center text-gray-400 dark:text-slate-500 space-y-2"
    >
      <div class="w-12 h-12 rounded-2xl bg-gray-100 dark:bg-white/[0.04] flex items-center justify-center">
        <FbIcon name="info" size="22px" class="opacity-40" />
      </div>
      <span class="text-xs font-medium">Select a file to inspect metadata and preview</span>
    </div>

    <!-- Inspector Content for Selected Entry -->
    <div v-else class="flex-1 overflow-y-auto flex flex-col p-4 space-y-4 scrollbar-thin">
      <!-- Live Preview Canvas -->
      <div class="w-full rounded-2xl border border-gray-200/90 dark:border-white/[0.08] bg-white dark:bg-[#11141b] overflow-hidden flex flex-col items-center justify-center min-h-[160px] max-h-[260px] relative shadow-2xs">
        <!-- Image Preview -->
        <template v-if="isImage(entry)">
          <img
            :src="getReadUrl(entry)"
            :alt="entry.name"
            class="max-h-[240px] w-auto max-w-full object-contain p-2 rounded-xl"
            loading="lazy"
          />
        </template>

        <!-- Video Preview -->
        <template v-else-if="isVideo(entry)">
          <video
            controls
            :src="getReadUrl(entry)"
            class="max-h-[240px] w-full object-contain bg-black rounded-xl"
          ></video>
        </template>

        <!-- Audio Preview -->
        <template v-else-if="isAudio(entry)">
          <div class="p-4 w-full flex flex-col items-center space-y-3">
            <div class="w-12 h-12 rounded-2xl bg-gradient-to-tr from-emerald-500/20 to-teal-500/20 text-emerald-500 flex items-center justify-center shadow-xs">
              <FbIcon name="audio" size="24px" />
            </div>
            <audio controls :src="getReadUrl(entry)" class="w-full h-8"></audio>
          </div>
        </template>

        <!-- Text / Code Preview -->
        <template v-else-if="isText(entry)">
          <div class="w-full h-full max-h-[250px] flex flex-col">
            <div class="px-3 py-1.5 bg-gray-50 dark:bg-white/[0.03] border-b border-gray-200/60 dark:border-white/[0.06] text-[10px] text-gray-400 font-mono flex items-center justify-between">
              <span>Text Preview</span>
              <span v-if="isLoadingText" class="text-blue-500 animate-pulse">Loading text...</span>
            </div>
            <div class="p-2.5 flex-1 overflow-auto font-mono text-[11px] leading-relaxed text-gray-800 dark:text-slate-200 select-text whitespace-pre-wrap break-all bg-gray-50/30 dark:bg-black/20">
              {{ textContent || (isLoadingText ? 'Scanning entry content...' : 'Empty file') }}
            </div>
          </div>
        </template>

        <!-- Generic / Binary / Folder Preview Badge -->
        <template v-else>
          <div class="p-6 flex flex-col items-center justify-center space-y-2 text-center">
            <div class="w-14 h-14 rounded-2xl bg-gray-100 dark:bg-white/[0.04] flex items-center justify-center text-gray-500 dark:text-slate-400 shadow-2xs">
              <FbIcon :name="getIconName(entry)" size="28px" />
            </div>
            <span class="font-bold text-xs text-gray-800 dark:text-slate-200 truncate max-w-[200px]">{{ entry.name }}</span>
            <span class="text-[10px] font-mono text-gray-400 uppercase tracking-wider">{{ getFileExt(entry.name) }}</span>
          </div>
        </template>
      </div>

      <!-- File Identity -->
      <div class="flex flex-col space-y-1">
        <h4 class="font-bold text-xs sm:text-sm text-gray-900 dark:text-slate-100 break-all leading-snug">
          {{ entry.name }}
        </h4>
        <div class="flex items-center space-x-2 text-[11px] text-gray-400 font-mono">
          <span class="truncate" :title="entry.path">{{ entry.path }}</span>
          <button
            type="button"
            @click="copyPath(entry.path)"
            class="p-0.5 hover:text-blue-500 transition cursor-pointer shrink-0"
            title="Copy path"
          >
            <FbIcon name="copy" size="12px" />
          </button>
        </div>
      </div>

      <!-- Compression & Metadata Card -->
      <div class="rounded-2xl border border-gray-200/80 dark:border-white/[0.08] bg-white dark:bg-[#11141b] p-3 space-y-2.5 shadow-2xs">
        <span class="text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider block">
          Compression & Stats
        </span>

        <div class="grid grid-cols-2 gap-2 text-xs">
          <!-- Uncompressed Size -->
          <div class="p-2 rounded-xl bg-gray-50 dark:bg-white/[0.03] border border-gray-100 dark:border-white/[0.04]">
            <span class="text-[10px] text-gray-400 dark:text-slate-500 block">Original Size</span>
            <span class="font-mono font-bold text-gray-800 dark:text-slate-200 text-xs mt-0.5 block">
              {{ formatBytes(entry.size || 0) }}
            </span>
          </div>

          <!-- Compressed Size -->
          <div class="p-2 rounded-xl bg-gray-50 dark:bg-white/[0.03] border border-gray-100 dark:border-white/[0.04]">
            <span class="text-[10px] text-gray-400 dark:text-slate-500 block">Compressed</span>
            <span class="font-mono font-bold text-gray-800 dark:text-slate-200 text-xs mt-0.5 block">
              {{ entry.compressed_size ? formatBytes(entry.compressed_size) : '—' }}
            </span>
          </div>
        </div>

        <!-- Saved Efficiency Ratio -->
        <div
          v-if="calcSavedPercent(entry.size, entry.compressed_size) > 0"
          class="flex items-center justify-between p-2 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-700 dark:text-emerald-400 text-xs"
        >
          <span class="font-medium text-[11px]">Storage Saved</span>
          <span class="font-bold font-mono text-xs">{{ calcSavedPercent(entry.size, entry.compressed_size) }}%</span>
        </div>

        <!-- Modified Date -->
        <div class="flex items-center justify-between text-[11px] pt-1 border-t border-gray-100 dark:border-white/[0.04]">
          <span class="text-gray-400 dark:text-slate-500">Modified:</span>
          <span class="font-mono text-gray-600 dark:text-slate-300">{{ formatModifiedDate(entry.modified_at) }}</span>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="flex flex-col space-y-2 pt-1">
        <button
          type="button"
          @click="$emit('extract', [entry.path])"
          class="w-full py-2 px-3 rounded-xl bg-blue-600 hover:bg-blue-500 active:scale-[0.98] text-white font-semibold flex items-center justify-center space-x-2 shadow-xs transition cursor-pointer text-xs duration-150"
        >
          <FbIcon name="download" size="14px" />
          <span>Extract This File</span>
        </button>

        <a
          :href="getReadUrl(entry)"
          :download="entry.name"
          class="w-full py-2 px-3 rounded-xl border border-gray-200 dark:border-white/[0.1] hover:bg-gray-100 dark:hover:bg-white/[0.05] active:scale-[0.98] text-gray-700 dark:text-slate-200 font-medium flex items-center justify-center space-x-2 transition cursor-pointer text-xs duration-150"
        >
          <FbIcon name="external-link" size="13px" />
          <span>Download Raw Entry</span>
        </a>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../../common/FbIcon.vue';
import type { IconName } from '../../../utils/icons';
import type { VirtualArchiveEntry } from '../../../api/archive';
import { getArchiveEntryReadUrl, readArchiveEntryTextApi } from '../../../api/archive';
import { useUiStore } from '../../../stores/uiStore';

const props = defineProps<{
  connectionId: string;
  archivePath: string;
  entry: VirtualArchiveEntry | null;
}>();

defineEmits<{
  (e: 'collapse'): void;
  (e: 'extract', paths: string[]): void;
}>();

const uiStore = useUiStore();
const textContent = ref('');
const isLoadingText = ref(false);

watch(
  () => props.entry,
  async (newEntry) => {
    textContent.value = '';
    if (newEntry && isText(newEntry) && (newEntry.size || 0) < 500000) {
      isLoadingText.value = true;
      try {
        textContent.value = await readArchiveEntryTextApi(
          props.connectionId,
          props.archivePath,
          newEntry.path
        );
      } catch {
        textContent.value = 'Failed to load preview text';
      } finally {
        isLoadingText.value = false;
      }
    }
  },
  { immediate: true }
);

function getReadUrl(entry: VirtualArchiveEntry): string {
  return getArchiveEntryReadUrl(props.connectionId, props.archivePath, entry.path);
}

function getFileExt(name: string): string {
  return name.split('.').pop() || 'file';
}

function isImage(entry: VirtualArchiveEntry): boolean {
  const ext = getFileExt(entry.name).toLowerCase();
  return ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico', 'avif'].includes(ext);
}

function isVideo(entry: VirtualArchiveEntry): boolean {
  const ext = getFileExt(entry.name).toLowerCase();
  return ['mp4', 'webm', 'mov', 'mkv', 'avi'].includes(ext);
}

function isAudio(entry: VirtualArchiveEntry): boolean {
  const ext = getFileExt(entry.name).toLowerCase();
  return ['mp3', 'wav', 'ogg', 'flac', 'm4a'].includes(ext);
}

function isText(entry: VirtualArchiveEntry): boolean {
  if (entry.name.startsWith('.')) return true;
  const ext = getFileExt(entry.name).toLowerCase();
  const textExts = [
    'txt', 'md', 'log', 'env', 'json', 'yaml', 'yml', 'toml', 'xml', 'csv', 'tsv',
    'rs', 'ts', 'js', 'jsx', 'tsx', 'vue', 'html', 'css', 'scss', 'py', 'sh', 'c', 'cpp',
    'go', 'java', 'sql', 'conf', 'ini', 'dockerfile', 'lock'
  ];
  return textExts.includes(ext);
}

function getIconName(entry: VirtualArchiveEntry): IconName {
  if (entry.kind === 'directory') return 'folder';
  if (isImage(entry)) return 'image';
  if (isVideo(entry)) return 'video';
  if (isAudio(entry)) return 'audio';
  if (isText(entry)) return 'code';
  return 'file';
}

function calcSavedPercent(original: number, compressed?: number | null): number {
  if (!compressed || original <= 0 || compressed >= original) return 0;
  return Math.round(((original - compressed) / original) * 100);
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatModifiedDate(isoString?: string | null): string {
  if (!isoString) return '—';
  try {
    const d = new Date(isoString);
    return d.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return '—';
  }
}

async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    uiStore.showToast('Path copied to clipboard', 'success');
  } catch {
    uiStore.showToast('Failed to copy path', 'error');
  }
}
</script>
