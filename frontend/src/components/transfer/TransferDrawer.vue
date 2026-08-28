<template>
  <div v-if="transferStore.isDrawerOpen || transferStore.jobs.length > 0">
    <!-- Mobile Backdrop Overlay -->
    <div
      v-if="uiStore.isMobile && transferStore.isDrawerOpen"
      @click="transferStore.isDrawerOpen = false"
      class="fixed inset-0 bg-black/60 backdrop-blur-xs z-40 animate-in fade-in duration-150"
    ></div>

    <!-- Container: Desktop Floating Button / Card (Safely above footer status bar) vs Mobile Bottom Sheet -->
    <div
      :class="[
        'z-40 select-none font-sans text-xs',
        uiStore.isMobile
          ? (transferStore.isDrawerOpen ? 'fixed inset-x-0 bottom-0 z-50' : 'hidden')
          : 'fixed bottom-12 right-5 flex flex-col items-end'
      ]"
    >
      <!-- Collapsed Floating Button (Desktop Only) -->
      <button
        v-if="!transferStore.isDrawerOpen && !uiStore.isMobile"
        @click="transferStore.isDrawerOpen = true"
        class="flex items-center space-x-2 bg-white/95 dark:bg-slate-900/95 hover:bg-white dark:hover:bg-slate-800 border border-gray-200/90 dark:border-slate-700/90 text-gray-800 dark:text-slate-100 px-3.5 py-2 rounded-full shadow-2xl backdrop-blur-md transition-all cursor-pointer hover:scale-105 active:scale-95 group"
        title="Open Transfer Manager"
      >
        <span class="text-sm">⚡</span>
        <span class="font-semibold text-xs">Transfers</span>
        <span
          v-if="transferStore.activeCount > 0"
          class="px-2 py-0.5 rounded-full bg-blue-600 text-white font-bold text-[10px] animate-pulse flex items-center space-x-1"
        >
          <span>{{ transferStore.activeCount }}</span>
          <span v-if="transferStore.totalSpeedBytesPerSec > 0" class="text-[9px] font-normal opacity-90">
            · {{ formatSpeed(transferStore.totalSpeedBytesPerSec) }}
          </span>
        </span>
        <span
          v-else-if="transferStore.jobs.length > 0"
          class="px-1.5 py-0.2 rounded-full bg-gray-200 dark:bg-slate-800 text-gray-600 dark:text-slate-400 font-bold text-[10px]"
        >
          {{ transferStore.jobs.length }}
        </span>
      </button>

      <!-- Expanded Drawer Panel (Bottom Sheet on Mobile, Floating Glass Card on Desktop) -->
      <div
        v-else-if="transferStore.isDrawerOpen"
        :class="[
          'bg-white/95 dark:bg-[#0f1422]/95 border border-gray-200 dark:border-slate-800 shadow-2xl backdrop-blur-md overflow-hidden flex flex-col',
          uiStore.isMobile
            ? 'w-full rounded-t-3xl max-h-[80vh] pb-safe animate-in slide-in-from-bottom duration-200'
            : 'w-84 sm:w-96 rounded-2xl animate-in slide-in-from-bottom-3 duration-150'
        ]"
      >
        <!-- Mobile Drag Indicator -->
        <div v-if="uiStore.isMobile" class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto mt-3 mb-1"></div>

        <!-- Header with Refresh, Clear, and Close -->
        <div class="p-3.5 bg-gray-50/90 dark:bg-slate-950/90 border-b border-gray-200 dark:border-slate-800/80 flex items-center justify-between">
          <div class="flex items-center space-x-2">
            <span class="text-sm">⚡</span>
            <span class="font-bold text-gray-900 dark:text-white">Transfer Manager</span>
            <span
              v-if="transferStore.activeCount > 0"
              class="px-1.5 py-0.2 rounded-full bg-blue-500/20 text-blue-600 dark:text-blue-400 font-bold text-[10px] animate-pulse"
            >
              {{ transferStore.activeCount }} active
            </span>
          </div>

          <div class="flex items-center space-x-1">
            <!-- Manual Sync / Refresh Button -->
            <button
              @click="handleRefresh"
              :disabled="transferStore.isRefreshing"
              class="p-1 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
              title="Refresh transfer list"
            >
              <FbIcon name="refresh" size="13px" :class="{ 'animate-spin': transferStore.isRefreshing }" />
            </button>

            <!-- Clear Finished Button -->
            <button
              v-if="hasFinishedJobs"
              @click="handleClear"
              class="text-[10px] text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white px-2 py-0.5 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer font-medium"
              title="Clear completed and failed transfers"
            >
              Clear
            </button>

            <!-- Close Drawer Button -->
            <button
              @click="transferStore.isDrawerOpen = false"
              class="p-1 rounded-lg text-gray-400 hover:text-gray-900 dark:hover:text-white transition cursor-pointer font-bold text-xs"
              title="Close Drawer"
            >
              ✕
            </button>
          </div>
        </div>

        <!-- Transfer Jobs List with Active & Finished grouping -->
        <div class="max-h-80 overflow-y-auto p-3 space-y-3">
          <div v-if="transferStore.jobs.length === 0" class="py-8 text-center text-gray-400 dark:text-slate-500 text-xs font-medium flex flex-col items-center justify-center space-y-1">
            <span class="text-xl">✨</span>
            <span>No active transfers</span>
          </div>

          <!-- Active Transfers Section -->
          <div v-if="activeJobs.length > 0" class="space-y-2">
            <div class="text-[10px] font-bold text-blue-600 dark:text-blue-400 uppercase tracking-wider flex items-center justify-between">
              <span>Active ({{ activeJobs.length }})</span>
            </div>

            <div
              v-for="job in activeJobs"
              :key="job.id"
              class="p-2 rounded-xl bg-blue-50/50 dark:bg-blue-950/20 border border-blue-100 dark:border-blue-900/40 space-y-1.5 group/item"
            >
              <div class="flex items-center justify-between text-[11px]">
                <div class="flex items-center space-x-1.5 truncate max-w-[210px]">
                  <FbIcon
                    :name="getTransferTypeIcon(job.transfer_type)"
                    size="13px"
                    :class="[
                      job.status === 'running' ? 'text-blue-500 animate-bounce' : 'text-gray-400 dark:text-slate-500'
                    ]"
                  />
                  <span class="font-semibold text-gray-800 dark:text-slate-200 truncate" :title="job.name">
                    {{ job.name }}
                  </span>
                </div>

                <div class="flex items-center space-x-1.5 shrink-0">
                  <span
                    :class="[
                      'text-[9px] font-bold uppercase px-1.5 py-0.2 rounded-md',
                      job.status === 'running' ? 'bg-blue-500/20 text-blue-600 dark:text-blue-400 animate-pulse' : 'bg-amber-500/20 text-amber-600 dark:text-amber-400'
                    ]"
                  >
                    {{ job.status }}
                  </span>

                  <!-- Cancel button -->
                  <button
                    @click="transferStore.cancelTransfer(job.id)"
                    class="text-red-500 hover:text-red-600 font-bold px-1 cursor-pointer text-[11px]"
                    title="Cancel Transfer"
                  >
                    ✕
                  </button>
                </div>
              </div>

              <!-- Progress Bar -->
              <div class="w-full bg-gray-200/80 dark:bg-slate-800 rounded-full h-1.5 overflow-hidden">
                <div
                  class="h-1.5 transition-all duration-200 rounded-full bg-gradient-to-r from-blue-600 to-sky-400"
                  :style="{ width: `${calculatePercent(job)}%` }"
                ></div>
              </div>

              <!-- Transfer Meta -->
              <div class="flex items-center justify-between text-[10px] text-gray-500 dark:text-slate-400 font-mono">
                <span>{{ formatBytes(job.transferred_bytes) }} / {{ formatBytes(job.total_bytes) }} ({{ calculatePercent(job) }}%)</span>
                <span v-if="job.status === 'running'">
                  {{ getLiveSpeed(job) }}
                </span>
              </div>
            </div>
          </div>

          <!-- Finished / History Section -->
          <div v-if="finishedJobs.length > 0" class="space-y-2">
            <div class="text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider flex items-center justify-between pt-1 border-t border-gray-100 dark:border-slate-800">
              <span>History ({{ finishedJobs.length }})</span>
              <button
                @click="handleClear"
                class="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer font-semibold normal-case text-[10px]"
              >
                Clear History
              </button>
            </div>

            <div
              v-for="job in finishedJobs"
              :key="job.id"
              class="p-2 rounded-xl bg-gray-50/60 dark:bg-slate-900/40 border border-gray-100 dark:border-slate-800 space-y-1.5 group/item"
            >
              <div class="flex items-center justify-between text-[11px]">
                <div class="flex items-center space-x-1.5 truncate max-w-[210px]">
                  <FbIcon
                    :name="getTransferTypeIcon(job.transfer_type)"
                    size="13px"
                    class="text-gray-400 dark:text-slate-500"
                  />
                  <span class="font-medium text-gray-700 dark:text-slate-300 truncate" :title="job.name">
                    {{ job.name }}
                  </span>
                </div>

                <div class="flex items-center space-x-1.5 shrink-0">
                  <span
                    :class="[
                      'text-[9px] font-bold uppercase px-1.5 py-0.2 rounded-md',
                      job.status === 'completed' ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400' : '',
                      job.status === 'interrupted' ? 'bg-amber-500/20 text-amber-600 dark:text-amber-400' : '',
                      job.status === 'failed' ? 'bg-red-500/20 text-red-600 dark:text-red-400' : '',
                      job.status === 'cancelled' ? 'bg-gray-200/60 dark:bg-slate-800 text-gray-500 dark:text-slate-400' : '',
                    ]"
                  >
                    {{ job.status }}
                  </span>

                  <!-- Retry for failed / cancelled / interrupted -->
                  <button
                    v-if="job.status === 'failed' || job.status === 'cancelled' || job.status === 'interrupted'"
                    @click="transferStore.retryTransfer(job.id)"
                    class="text-blue-600 dark:text-blue-400 hover:underline font-medium px-1 cursor-pointer text-[10px]"
                    title="Retry Transfer"
                  >
                    Retry
                  </button>

                  <!-- Dismiss single item -->
                  <button
                    @click="transferStore.removeJob(job.id)"
                    class="opacity-0 group-hover/item:opacity-100 text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 px-1 cursor-pointer text-[10px] transition"
                    title="Dismiss from history"
                  >
                    ✕
                  </button>
                </div>
              </div>

              <!-- Progress Bar -->
              <div class="w-full bg-gray-200/60 dark:bg-slate-800 rounded-full h-1 overflow-hidden">
                <div
                  :class="[
                    'h-1 rounded-full',
                    job.status === 'completed'
                      ? 'bg-emerald-500'
                      : (job.status === 'interrupted'
                          ? 'bg-amber-500'
                          : (job.status === 'failed' ? 'bg-red-500' : 'bg-gray-400 dark:bg-slate-600'))
                  ]"
                  :style="{ width: `${calculatePercent(job)}%` }"
                ></div>
              </div>

              <!-- Transfer Meta -->
              <div class="flex items-center justify-between text-[10px] text-gray-400 dark:text-slate-500 font-mono">
                <span>{{ formatBytes(job.transferred_bytes) }} / {{ formatBytes(job.total_bytes) }}</span>
                <span>{{ calculatePercent(job) }}%</span>
              </div>

              <!-- Error message if failed or interrupted -->
              <div
                v-if="(job.status === 'failed' || job.status === 'interrupted') && job.error_message"
                :class="job.status === 'interrupted' ? 'text-amber-600 dark:text-amber-400' : 'text-red-500 dark:text-red-400'"
                class="text-[10px] truncate"
              >
                {{ job.error_message }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTransferStore } from '../../stores/transferStore';
import { useUiStore } from '../../stores/uiStore';
import FbIcon from '../common/FbIcon.vue';
import type { TransferJob, TransferType } from '../../types/transfer';
import type { IconName } from '../../utils/icons';

const transferStore = useTransferStore();
const uiStore = useUiStore();

const activeJobs = computed(() => {
  return transferStore.jobs.filter(
    (j) =>
      j.status === 'running' ||
      j.status === 'queued' ||
      j.status === 'cancellation_requested'
  );
});

const finishedJobs = computed(() => {
  return transferStore.jobs.filter(
    (j) =>
      j.status === 'completed' ||
      j.status === 'failed' ||
      j.status === 'cancelled' ||
      j.status === 'interrupted'
  );
});

const hasFinishedJobs = computed(() => finishedJobs.value.length > 0);

function calculatePercent(job: TransferJob): number {
  if (job.total_bytes === 0) return job.status === 'completed' ? 100 : 0;
  return Math.min(100, Math.round((job.transferred_bytes * 100) / job.total_bytes));
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatSpeed(bytesPerSec: number): string {
  if (!bytesPerSec || bytesPerSec === 0) return '0 KB/s';
  const mbps = bytesPerSec / (1024 * 1024);
  if (mbps >= 1) {
    return `${mbps.toFixed(1)} MB/s`;
  }
  const kbps = bytesPerSec / 1024;
  return `${kbps.toFixed(0)} KB/s`;
}

function getLiveSpeed(job: TransferJob): string {
  const metric = transferStore.speedMetrics[job.id];
  const speed = metric?.speedBytesPerSec || job.speed_bytes_per_sec || 0;
  const eta = metric?.etaSeconds !== undefined ? metric.etaSeconds : job.eta_seconds;

  let str = formatSpeed(speed);
  if (eta !== null && eta !== undefined && eta > 0) {
    str += ` • ETA ${eta}s`;
  }
  return str;
}

function getTransferTypeIcon(type: TransferType): IconName {
  if (type === 'upload') return 'upload';
  if (type === 'move') return 'move';
  return 'copy';
}

async function handleRefresh() {
  await transferStore.refreshJobs();
  uiStore.showToast('Transfer list synced', 'info');
}

function handleClear() {
  transferStore.clearFinished();
  uiStore.showToast('Cleared finished transfers', 'info');
}
</script>

