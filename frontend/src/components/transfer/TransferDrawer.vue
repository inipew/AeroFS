<template>
  <div v-if="transferStore.isDrawerOpen || !uiStore.isMobile">
    <!-- Mobile Backdrop Overlay -->
    <div
      v-if="uiStore.isMobile && transferStore.isDrawerOpen"
      @click="transferStore.isDrawerOpen = false"
      class="fixed inset-0 bg-black/60 backdrop-blur-xs z-40 animate-in fade-in duration-150"
    ></div>

    <!-- Container: Desktop Floating Button / Card vs Mobile Bottom Sheet -->
    <div
      :class="[
        'z-40 select-none font-sans text-xs',
        uiStore.isMobile
          ? (transferStore.isDrawerOpen ? 'fixed inset-x-0 bottom-0 z-50' : 'hidden')
          : 'fixed bottom-4 right-4 flex flex-col items-end'
      ]"
    >
      <!-- Collapsed Floating Button (Desktop Only) -->
      <button
        v-if="!transferStore.isDrawerOpen && !uiStore.isMobile"
        @click="transferStore.isDrawerOpen = true"
        class="flex items-center space-x-2 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 border border-gray-200 dark:border-slate-700 text-gray-800 dark:text-slate-200 px-4 py-2.5 rounded-full shadow-2xl transition cursor-pointer"
      >
        <span class="text-sm">⚡</span>
        <span class="font-semibold">Transfers</span>
        <span
          v-if="transferStore.activeCount > 0"
          class="px-2 py-0.5 rounded-full bg-blue-600 text-white font-bold text-[10px] animate-pulse"
        >
          {{ transferStore.activeCount }}
        </span>
      </button>

      <!-- Expanded Drawer Panel (Bottom Sheet on Mobile, Floating Card on Desktop) -->
      <div
        v-else-if="transferStore.isDrawerOpen"
        :class="[
          'bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 shadow-2xl overflow-hidden flex flex-col',
          uiStore.isMobile
            ? 'w-full rounded-t-3xl max-h-[80vh] pb-safe animate-in slide-in-from-bottom duration-200'
            : 'w-80 sm:w-96 rounded-2xl animate-in slide-in-from-bottom-3 duration-150'
        ]"
      >
        <!-- Mobile Drag Indicator -->
        <div v-if="uiStore.isMobile" class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto mt-3 mb-1"></div>

        <!-- Header -->
        <div class="p-3.5 bg-gray-50/80 dark:bg-slate-950/80 border-b border-gray-200 dark:border-slate-800/80 flex items-center justify-between">
          <div class="flex items-center space-x-2">
            <span class="text-sm">⚡</span>
            <span class="font-bold text-gray-900 dark:text-white">Transfer Manager</span>
            <span
              v-if="transferStore.activeCount > 0"
              class="px-1.5 py-0.2 rounded bg-blue-500/20 text-blue-600 dark:text-blue-400 font-bold text-[10px]"
            >
              {{ transferStore.activeCount }} active
            </span>
          </div>

          <div class="flex items-center space-x-1">
            <button
              @click="transferStore.clearFinished()"
              class="text-[10px] text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white px-2 py-0.5 rounded hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
              title="Clear completed"
            >
              Clear
            </button>
            <button
              @click="transferStore.isDrawerOpen = false"
              class="text-gray-400 hover:text-gray-900 dark:hover:text-white p-1 text-sm cursor-pointer"
            >
              ✕
            </button>
          </div>
        </div>

        <!-- Transfer Jobs List -->
        <div class="max-h-72 overflow-y-auto p-3 space-y-3 divide-y divide-gray-100 dark:divide-slate-800/60">
          <div v-if="transferStore.jobs.length === 0" class="py-8 text-center text-gray-400 dark:text-slate-500 text-xs font-medium">
            No active transfers
          </div>

          <div
            v-for="job in transferStore.jobs"
            :key="job.id"
            class="pt-2 first:pt-0 space-y-1.5"
          >
            <div class="flex items-center justify-between text-[11px]">
              <span class="font-semibold text-gray-800 dark:text-slate-200 truncate max-w-[200px]" :title="job.name">
                {{ job.name }}
              </span>
              <div class="flex items-center space-x-1.5 shrink-0">
                <span
                  :class="[
                    'text-[10px] font-bold uppercase px-1.5 py-0.2 rounded',
                    job.status === 'running' ? 'bg-blue-500/20 text-blue-600 dark:text-blue-400 animate-pulse' : '',
                    job.status === 'completed' ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400' : '',
                    job.status === 'failed' ? 'bg-red-500/20 text-red-600 dark:text-red-400' : '',
                    job.status === 'cancelled' ? 'bg-gray-200/60 dark:bg-slate-800 text-gray-500 dark:text-slate-400' : '',
                    job.status === 'queued' ? 'bg-amber-500/20 text-amber-600 dark:text-amber-400' : '',
                  ]"
                >
                  {{ job.status }}
                </span>

                <button
                  v-if="job.status === 'failed' || job.status === 'cancelled'"
                  @click="transferStore.retryTransfer(job.id)"
                  class="text-blue-500 hover:text-blue-600 font-medium px-1 cursor-pointer text-[10px]"
                  title="Retry Transfer"
                >
                  Retry
                </button>

                <button
                  v-if="job.status === 'running' || job.status === 'queued'"
                  @click="transferStore.cancelTransfer(job.id)"
                  class="text-red-500 hover:text-red-600 font-bold px-1 cursor-pointer"
                  title="Cancel Transfer"
                >
                  ✕
                </button>
              </div>
            </div>

            <!-- Progress Bar -->
            <div class="w-full bg-gray-100 dark:bg-slate-800 rounded-full h-1.5 overflow-hidden">
              <div
                :class="[
                  'h-1.5 transition-all duration-200 rounded-full',
                  job.status === 'completed' ? 'bg-emerald-500' : (job.status === 'failed' ? 'bg-red-500' : 'bg-blue-600')
                ]"
                :style="{ width: `${calculatePercent(job)}%` }"
              ></div>
            </div>

            <!-- Transfer Meta (Transferred bytes, percent, speed, ETA) -->
            <div class="flex items-center justify-between text-[10px] text-gray-400 dark:text-slate-500 font-mono">
              <span>{{ formatBytes(job.transferred_bytes) }} / {{ formatBytes(job.total_bytes) }} ({{ calculatePercent(job) }}%)</span>
              <span v-if="job.status === 'running'">
                {{ getLiveSpeed(job) }}
              </span>
            </div>

            <!-- Error message if failed -->
            <div v-if="job.status === 'failed' && job.error_message" class="text-[10px] text-red-500 dark:text-red-400 truncate">
              {{ job.error_message }}
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useTransferStore } from '../../stores/transferStore';
import { useUiStore } from '../../stores/uiStore';
import type { TransferJob } from '../../types/transfer';

const transferStore = useTransferStore();
const uiStore = useUiStore();

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
</script>
