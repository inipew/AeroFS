<template>
  <div class="fixed bottom-4 right-4 z-40 flex flex-col items-end select-none font-sans text-xs">
    <!-- Collapsed Floating Button -->
    <button
      v-if="!transferStore.isDrawerOpen"
      @click="transferStore.isDrawerOpen = true"
      class="flex items-center space-x-2 bg-slate-900 hover:bg-slate-800 border border-slate-700 text-slate-200 px-3.5 py-2 rounded-full shadow-2xl transition"
    >
      <span class="text-sm">⚡</span>
      <span class="font-medium">Transfers</span>
      <span
        v-if="transferStore.activeCount > 0"
        class="px-1.5 py-0.2 rounded-full bg-indigo-500 text-white font-bold text-[10px] animate-pulse"
      >
        {{ transferStore.activeCount }}
      </span>
    </button>

    <!-- Expanded Drawer Panel -->
    <div
      v-else
      class="bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl w-80 sm:w-96 overflow-hidden flex flex-col animate-in slide-in-from-bottom-3 duration-150"
    >
      <!-- Header -->
      <div class="p-3 bg-slate-950/80 border-b border-slate-800 flex items-center justify-between">
        <div class="flex items-center space-x-2">
          <span class="text-sm">⚡</span>
          <span class="font-semibold text-white">Transfer Manager</span>
          <span
            v-if="transferStore.activeCount > 0"
            class="px-1.5 py-0.2 rounded bg-indigo-500/20 text-indigo-300 font-medium text-[10px]"
          >
            {{ transferStore.activeCount }} active
          </span>
        </div>

        <div class="flex items-center space-x-1">
          <button
            @click="transferStore.clearFinished()"
            class="text-[10px] text-slate-400 hover:text-white px-2 py-0.5 rounded hover:bg-slate-800 transition"
            title="Clear completed"
          >
            Clear
          </button>
          <button
            @click="transferStore.isDrawerOpen = false"
            class="text-slate-400 hover:text-white px-2 py-0.5 text-sm"
          >
            &times;
          </button>
        </div>
      </div>

      <!-- Transfer Jobs List -->
      <div class="max-h-72 overflow-y-auto p-3 space-y-3 bg-slate-900/60 divide-y divide-slate-800/60">
        <div v-if="transferStore.jobs.length === 0" class="py-8 text-center text-slate-500 text-xs">
          No transfers yet
        </div>

        <div
          v-for="job in transferStore.jobs"
          :key="job.id"
          class="pt-2 first:pt-0 space-y-1.5"
        >
          <div class="flex items-center justify-between text-[11px]">
            <span class="font-medium text-slate-200 truncate max-w-[200px]" :title="job.name">
              {{ job.name }}
            </span>
            <div class="flex items-center space-x-1.5">
              <span
                :class="[
                  'text-[10px] font-semibold uppercase px-1.5 py-0.2 rounded',
                  job.status === 'running' ? 'bg-indigo-500/20 text-indigo-300 animate-pulse' : '',
                  job.status === 'completed' ? 'bg-emerald-500/20 text-emerald-300' : '',
                  job.status === 'failed' ? 'bg-red-500/20 text-red-300' : '',
                  job.status === 'cancelled' ? 'bg-slate-700/40 text-slate-400' : '',
                  job.status === 'queued' ? 'bg-amber-500/20 text-amber-300' : '',
                ]"
              >
                {{ job.status }}
              </span>

              <button
                v-if="job.status === 'running' || job.status === 'queued'"
                @click="transferStore.cancelTransfer(job.id)"
                class="text-red-400 hover:text-red-300 font-bold px-1"
                title="Cancel Transfer"
              >
                &times;
              </button>
            </div>
          </div>

          <!-- Progress Bar -->
          <div class="w-full bg-slate-950 rounded-full h-1.5 overflow-hidden">
            <div
              :class="[
                'h-1.5 transition-all duration-200',
                job.status === 'completed' ? 'bg-emerald-500' : (job.status === 'failed' ? 'bg-red-500' : 'bg-indigo-500')
              ]"
              :style="{ width: `${calculatePercent(job)}%` }"
            ></div>
          </div>

          <!-- Metrics / Speed / ETA -->
          <div class="flex items-center justify-between text-[10px] text-slate-500 font-mono">
            <span>
              {{ formatBytes(job.transferred_bytes) }} / {{ formatBytes(job.total_bytes) }} ({{ calculatePercent(job) }}%)
            </span>
            <span v-if="job.status === 'running'">
              {{ formatSpeed(job.speed_bytes_per_sec) }}
              <span v-if="job.eta_seconds !== undefined"> • ETA {{ job.eta_seconds }}s</span>
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useTransferStore } from '../../stores/transferStore';
import type { TransferJob } from '../../types/transfer';

const transferStore = useTransferStore();

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
</script>
