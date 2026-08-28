<template>
  <div v-if="transferStore.isDrawerOpen || transferStore.jobs.length > 0">
    <!-- Mobile Backdrop Overlay -->
    <Transition name="ios-fade">
      <div
        v-if="uiStore.isMobile && transferStore.isDrawerOpen"
        @click="transferStore.isDrawerOpen = false"
        class="fixed inset-0 bg-black/60 backdrop-blur-xs z-40"
      ></div>
    </Transition>

    <!-- Mobile Drawer (Bottom Sheet) -->
    <div
      v-if="uiStore.isMobile"
      :class="transferStore.isDrawerOpen ? 'fixed inset-x-0 bottom-0 z-50 select-none font-sans text-xs' : 'hidden'"
    >
      <Transition name="ios-mobile-sheet">
        <div
          v-if="transferStore.isDrawerOpen"
          class="w-full rounded-t-3xl max-h-[80vh] pb-safe bg-white/95 dark:bg-[#0b1329]/95 border-t border-x border-gray-200/90 dark:border-slate-800/90 shadow-2xl backdrop-blur-2xl overflow-hidden flex flex-col ring-1 ring-black/5 dark:ring-white/10"
        >
          <!-- Mobile Drag Handle -->
          <div
            @click="transferStore.isDrawerOpen = false"
            class="w-full pt-3 pb-1 cursor-pointer flex justify-center hover:opacity-80 active:scale-95 transition-transform"
          >
            <div class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full"></div>
          </div>

          <!-- Mobile Header Bar -->
          <div
            @click="toggleCollapse"
            class="p-3.5 bg-gray-50/90 dark:bg-slate-900/90 border-b border-gray-200/80 dark:border-slate-800/80 flex items-center justify-between cursor-pointer active:bg-gray-100 dark:active:bg-slate-800/60 transition-colors select-none"
          >
            <div class="flex items-center space-x-2.5">
              <div
                class="w-6 h-6 rounded-lg bg-amber-500/15 dark:bg-amber-500/20 text-amber-600 dark:text-amber-400 flex items-center justify-center text-xs shadow-xs"
                :class="{ 'animate-pulse': transferStore.activeCount > 0 }"
              >
                ⚡
              </div>
              <span class="font-bold text-gray-900 dark:text-slate-100 tracking-tight">Transfer Manager</span>
              <span
                v-if="transferStore.activeCount > 0"
                class="px-1.5 py-0.2 rounded-full bg-blue-500/15 text-blue-600 dark:text-blue-400 font-bold text-[10px]"
              >
                {{ transferStore.activeCount }} active
              </span>
            </div>

            <div class="flex items-center space-x-1" @click.stop>
              <button
                @click.stop="handleRefresh"
                :disabled="transferStore.isRefreshing"
                class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200"
              >
                <FbIcon name="refresh" size="13px" :class="{ 'animate-spin': transferStore.isRefreshing }" />
              </button>
              <button
                v-if="hasFinishedJobs"
                @click.stop="handleClear"
                class="text-[10px] text-gray-500 px-2 py-1"
              >
                Clear
              </button>
              <button
                @click.stop="transferStore.isDrawerOpen = false"
                class="p-1.5 rounded-lg text-gray-400"
              >
                <FbIcon name="x" size="13px" />
              </button>
            </div>
          </div>

          <!-- Body Content (Mobile) -->
          <div class="max-h-80 overflow-y-auto p-3 space-y-3 custom-scrollbar">
            <div
              v-if="transferStore.jobs.length === 0"
              class="py-8 px-4 text-center text-gray-400 dark:text-slate-500 text-xs font-medium flex flex-col items-center justify-center space-y-2 rounded-xl bg-gray-50/50 dark:bg-slate-900/30 border border-dashed border-gray-200 dark:border-slate-800/60"
            >
              <div class="w-10 h-10 rounded-full bg-blue-500/10 dark:bg-blue-500/15 text-blue-500 dark:text-blue-400 flex items-center justify-center text-lg shadow-inner">
                ⚡
              </div>
              <div class="space-y-0.5">
                <p class="font-semibold text-gray-700 dark:text-slate-300">No active transfers</p>
                <p class="text-[11px] text-gray-400 dark:text-slate-500">File transfers and operations will appear here.</p>
              </div>
            </div>

            <!-- Active Jobs (Mobile) -->
            <div v-if="activeJobs.length > 0" class="space-y-2">
              <div class="text-[10px] font-bold text-blue-600 dark:text-blue-400 uppercase tracking-wider flex items-center space-x-1.5 px-0.5">
                <span class="w-1.5 h-1.5 rounded-full bg-blue-500 animate-ping"></span>
                <span>In Progress ({{ activeJobs.length }})</span>
              </div>
              <div
                v-for="job in activeJobs"
                :key="job.id"
                class="p-2.5 rounded-xl bg-blue-50/50 dark:bg-blue-950/20 border border-blue-100/90 dark:border-blue-900/40 space-y-2"
              >
                <div class="flex items-center justify-between text-[11px]">
                  <div class="flex items-center space-x-2 truncate max-w-[210px]">
                    <FbIcon :name="getTransferTypeIcon(job.transfer_type)" size="12px" />
                    <span class="font-semibold text-gray-800 dark:text-slate-200 truncate">{{ job.name }}</span>
                  </div>
                  <button @click="transferStore.cancelTransfer(job.id)" class="text-gray-400 hover:text-red-500 p-0.5">
                    <FbIcon name="x" size="12px" />
                  </button>
                </div>
                <div class="w-full bg-gray-200/80 dark:bg-slate-800 rounded-full h-1.5 overflow-hidden">
                  <div class="h-1.5 rounded-full bg-gradient-to-r from-blue-600 to-cyan-400" :style="{ width: `${calculatePercent(job)}%` }"></div>
                </div>
                <div class="flex items-center justify-between text-[10px] text-gray-500 font-mono">
                  <span>{{ formatBytes(job.transferred_bytes) }} / {{ formatBytes(job.total_bytes) }} ({{ calculatePercent(job) }}%)</span>
                  <span>{{ getLiveSpeed(job) }}</span>
                </div>
              </div>
            </div>

            <!-- Finished Jobs (Mobile) -->
            <div v-if="finishedJobs.length > 0" class="space-y-2">
              <div class="text-[10px] font-bold text-gray-400 uppercase tracking-wider flex items-center justify-between pt-2 border-t border-gray-100 dark:border-slate-800/80 px-0.5">
                <span>History ({{ finishedJobs.length }})</span>
                <button @click="handleClear" class="text-blue-600 dark:text-blue-400 hover:underline">Clear History</button>
              </div>
              <div
                v-for="job in finishedJobs"
                :key="job.id"
                class="p-2.5 rounded-xl bg-gray-50/60 dark:bg-slate-900/40 border border-gray-100 dark:border-slate-800/80 space-y-1.5"
              >
                <div class="flex items-center justify-between text-[11px]">
                  <span class="font-medium text-gray-700 dark:text-slate-300 truncate">{{ job.name }}</span>
                  <button @click="transferStore.removeJob(job.id)" class="text-gray-400 p-0.5">
                    <FbIcon name="x" size="11px" />
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </div>

    <!-- Desktop Drawer & Pill (Fixed Anchor with Fluid Apple-style Spring Scale & Blur) -->
    <div
      v-else
      class="fixed bottom-12 right-5 z-40 select-none font-sans text-xs flex items-end justify-end pointer-events-none"
    >
      <div class="relative pointer-events-auto flex items-end justify-end">
        <!-- Floating Pill Button (Desktop) -->
        <Transition name="ios-pill">
          <button
            v-if="!transferStore.isDrawerOpen"
            @click="transferStore.isDrawerOpen = true"
            class="flex items-center space-x-2.5 bg-white/90 dark:bg-[#0f172a]/90 hover:bg-white dark:hover:bg-slate-800 border border-gray-200/80 dark:border-slate-700/80 text-gray-800 dark:text-slate-100 px-3.5 py-2 rounded-full shadow-2xl shadow-black/15 dark:shadow-black/40 backdrop-blur-xl cursor-pointer hover:scale-105 active:scale-95 group ring-1 ring-black/5 dark:ring-white/10 origin-bottom-right"
            title="Open Transfer Manager"
          >
            <span class="text-sm transform group-hover:scale-110 transition-transform">⚡</span>
            <span class="font-semibold text-xs tracking-tight">Transfers</span>

            <!-- Active count with pulsing glow badge -->
            <span
              v-if="transferStore.activeCount > 0"
              class="px-2 py-0.5 rounded-full bg-gradient-to-r from-blue-600 to-indigo-600 text-white font-bold text-[10px] shadow-xs shadow-blue-500/50 flex items-center space-x-1 animate-pulse"
            >
              <span class="relative flex h-1.5 w-1.5 mr-0.5">
                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-white"></span>
              </span>
              <span>{{ transferStore.activeCount }}</span>
              <span v-if="transferStore.totalSpeedBytesPerSec > 0" class="text-[9px] font-normal opacity-90">
                · {{ formatSpeed(transferStore.totalSpeedBytesPerSec) }}
              </span>
            </span>

            <!-- History count when idle -->
            <span
              v-else-if="transferStore.jobs.length > 0"
              class="px-1.5 py-0.2 rounded-full bg-gray-100 dark:bg-slate-800 text-gray-600 dark:text-slate-400 font-semibold text-[10px]"
            >
              {{ transferStore.jobs.length }}
            </span>
          </button>
        </Transition>

        <!-- Expanded Drawer Panel (Desktop Glass Card) -->
        <Transition name="ios-drawer">
          <div
            v-if="transferStore.isDrawerOpen"
            class="w-84 sm:w-96 rounded-2xl bg-white/95 dark:bg-[#0b1329]/95 border border-gray-200/80 dark:border-slate-800/80 shadow-2xl shadow-black/20 dark:shadow-black/60 backdrop-blur-2xl overflow-hidden flex flex-col ring-1 ring-black/5 dark:ring-white/10 origin-bottom-right"
          >
            <!-- Header Bar (Clicking anywhere collapses/minimizes) -->
            <div
              @click="toggleCollapse"
              class="p-3.5 bg-gray-50/90 dark:bg-slate-900/90 border-b border-gray-200/80 dark:border-slate-800/80 flex items-center justify-between cursor-pointer hover:bg-gray-100/90 dark:hover:bg-slate-800/60 active:scale-[0.99] transition-all group select-none"
              title="Click header to collapse / minimize"
            >
              <!-- Left: Title & Active Status -->
              <div class="flex items-center space-x-2.5">
                <div
                  class="w-6 h-6 rounded-lg bg-amber-500/15 dark:bg-amber-500/20 text-amber-600 dark:text-amber-400 flex items-center justify-center text-xs shadow-xs group-hover:scale-110 transition-transform"
                  :class="{ 'animate-pulse': transferStore.activeCount > 0 }"
                >
                  ⚡
                </div>
                <div class="flex flex-col">
                  <div class="flex items-center space-x-2">
                    <span class="font-bold text-gray-900 dark:text-slate-100 tracking-tight">Transfer Manager</span>
                    <span
                      v-if="transferStore.activeCount > 0"
                      class="px-1.5 py-0.2 rounded-full bg-blue-500/15 text-blue-600 dark:text-blue-400 font-bold text-[10px] flex items-center space-x-1"
                    >
                      <span class="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse"></span>
                      <span>{{ transferStore.activeCount }} active</span>
                    </span>
                  </div>
                </div>
              </div>

              <!-- Right: Controls -->
              <div class="flex items-center space-x-1" @click.stop>
                <!-- Manual Refresh / Sync Button -->
                <button
                  @click.stop="handleRefresh"
                  :disabled="transferStore.isRefreshing"
                  class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-200/70 dark:hover:bg-slate-800 transition cursor-pointer"
                  title="Sync transfer state"
                >
                  <FbIcon name="refresh" size="13px" :class="{ 'animate-spin': transferStore.isRefreshing }" />
                </button>

                <!-- Clear Finished History Button -->
                <button
                  v-if="hasFinishedJobs"
                  @click.stop="handleClear"
                  class="text-[10px] text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white px-2 py-1 rounded-lg hover:bg-gray-200/70 dark:hover:bg-slate-800 transition cursor-pointer font-medium"
                  title="Clear finished transfers"
                >
                  Clear
                </button>


                <!-- Close Button -->
                <button
                  @click.stop="transferStore.isDrawerOpen = false"
                  class="p-1.5 rounded-lg text-gray-400 hover:text-red-500 dark:hover:text-red-400 hover:bg-gray-200/70 dark:hover:bg-slate-800 transition cursor-pointer font-bold text-xs"
                  title="Close Drawer"
                >
                  <FbIcon name="x" size="13px" />
                </button>
              </div>
            </div>

            <!-- Transfer Jobs List -->
            <div class="max-h-80 overflow-y-auto p-3 space-y-3 custom-scrollbar">
              <!-- Empty State Illustration -->
              <div
                v-if="transferStore.jobs.length === 0"
                class="py-8 px-4 text-center text-gray-400 dark:text-slate-500 text-xs font-medium flex flex-col items-center justify-center space-y-2 rounded-xl bg-gray-50/50 dark:bg-slate-900/30 border border-dashed border-gray-200 dark:border-slate-800/60"
              >
                <div class="w-10 h-10 rounded-full bg-blue-500/10 dark:bg-blue-500/15 text-blue-500 dark:text-blue-400 flex items-center justify-center text-lg shadow-inner">
                  ⚡
                </div>
                <div class="space-y-0.5">
                  <p class="font-semibold text-gray-700 dark:text-slate-300">No active transfers</p>
                  <p class="text-[11px] text-gray-400 dark:text-slate-500">File copy, move, and download operations will appear here.</p>
                </div>
              </div>

              <!-- Active Transfers Section -->
              <div v-if="activeJobs.length > 0" class="space-y-2">
                <div class="text-[10px] font-bold text-blue-600 dark:text-blue-400 uppercase tracking-wider flex items-center justify-between px-0.5">
                  <span class="flex items-center space-x-1.5">
                    <span class="w-1.5 h-1.5 rounded-full bg-blue-500 animate-ping"></span>
                    <span>In Progress ({{ activeJobs.length }})</span>
                  </span>
                </div>

                <div
                  v-for="job in activeJobs"
                  :key="job.id"
                  class="p-2.5 rounded-xl bg-blue-50/50 dark:bg-blue-950/20 border border-blue-100/90 dark:border-blue-900/40 space-y-2 group/item shadow-xs hover:border-blue-300 dark:hover:border-blue-700/60 transition-all"
                >
                  <div class="flex items-center justify-between text-[11px]">
                    <div class="flex items-center space-x-2 truncate max-w-[210px]">
                      <div class="w-5 h-5 rounded-md bg-blue-500/15 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0">
                        <FbIcon
                          :name="getTransferTypeIcon(job.transfer_type)"
                          size="12px"
                          :class="[
                            job.status === 'running' ? 'animate-bounce' : 'text-gray-400 dark:text-slate-500'
                          ]"
                        />
                      </div>
                      <span class="font-semibold text-gray-800 dark:text-slate-200 truncate" :title="job.name">
                        {{ job.name }}
                      </span>
                    </div>

                    <div class="flex items-center space-x-1.5 shrink-0">
                      <span
                        :class="[
                          'text-[9px] font-bold uppercase px-1.5 py-0.5 rounded-md',
                          job.status === 'running' ? 'bg-blue-500/20 text-blue-600 dark:text-blue-400 animate-pulse' : 'bg-amber-500/20 text-amber-600 dark:text-amber-400'
                        ]"
                      >
                        {{ job.status === 'cancellation_requested' ? 'Cancelling...' : job.status }}
                      </span>

                      <button
                        v-if="job.status !== 'cancellation_requested'"
                        @click="transferStore.cancelTransfer(job.id)"
                        class="text-gray-400 hover:text-red-500 dark:hover:text-red-400 p-0.5 rounded-md hover:bg-red-50 dark:hover:bg-red-950/30 transition cursor-pointer"
                        title="Cancel Transfer"
                      >
                        <FbIcon name="x" size="12px" />
                      </button>
                    </div>
                  </div>

                  <!-- Progress Bar -->
                  <div class="w-full bg-gray-200/80 dark:bg-slate-800 rounded-full h-1.5 overflow-hidden">
                    <div
                      class="h-1.5 transition-all duration-200 rounded-full bg-gradient-to-r from-blue-600 via-indigo-500 to-cyan-400"
                      :style="{ width: `${calculatePercent(job)}%` }"
                    ></div>
                  </div>

                  <!-- Transfer Meta & Phase -->
                  <div class="flex items-center justify-between text-[10px] text-gray-500 dark:text-slate-400 font-mono">
                    <span>
                      {{ formatBytes(job.transferred_bytes) }} / {{ formatBytes(job.total_bytes) }} ({{ calculatePercent(job) }}%)
                      <span
                        v-if="job.phase && job.phase !== 'transferring' && job.phase !== 'completed'"
                        class="ml-1.5 font-sans font-semibold text-blue-600 dark:text-blue-400 animate-pulse"
                      >
                        • {{ formatPhase(job.phase) }}
                      </span>
                    </span>
                    <span v-if="job.status === 'running'" class="font-semibold text-gray-700 dark:text-slate-300">
                      {{ getLiveSpeed(job) }}
                    </span>
                  </div>
                </div>
              </div>

              <!-- Finished / History Section -->
              <div v-if="finishedJobs.length > 0" class="space-y-2">
                <div class="text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider flex items-center justify-between pt-2 border-t border-gray-100 dark:border-slate-800/80 px-0.5">
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
                  class="p-2.5 rounded-xl bg-gray-50/60 dark:bg-slate-900/40 border border-gray-100 dark:border-slate-800/80 space-y-1.5 group/item hover:border-gray-200 dark:hover:border-slate-700/60 transition-all"
                >
                  <div class="flex items-center justify-between text-[11px]">
                    <div class="flex items-center space-x-2 truncate max-w-[210px]">
                      <FbIcon
                        :name="getTransferTypeIcon(job.transfer_type)"
                        size="13px"
                        class="text-gray-400 dark:text-slate-500 shrink-0"
                      />
                      <span class="font-medium text-gray-700 dark:text-slate-300 truncate" :title="job.name">
                        {{ job.name }}
                      </span>
                    </div>

                    <div class="flex items-center space-x-1.5 shrink-0">
                      <span
                        :class="[
                          'text-[9px] font-bold uppercase px-1.5 py-0.5 rounded-md',
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
                        class="opacity-0 group-hover/item:opacity-100 text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 p-0.5 rounded-md hover:bg-gray-100 dark:hover:bg-slate-800 cursor-pointer transition"
                        title="Dismiss from history"
                      >
                        <FbIcon name="x" size="11px" />
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
                    class="text-[10px] truncate font-sans"
                    :title="job.error_message"
                  >
                    {{ job.error_message }}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Transition>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTransferStore } from '../../stores/transferStore';
import { useUiStore } from '../../stores/uiStore';
import FbIcon from '../common/FbIcon.vue';
import type { TransferJob, TransferPhase, TransferType } from '../../types/transfer';
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

function toggleCollapse() {
  transferStore.isDrawerOpen = !transferStore.isDrawerOpen;
}

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

function formatPhase(phase?: TransferPhase): string {
  switch (phase) {
    case 'preparing':
      return 'Preparing...';
    case 'finalizing':
      return 'Finalizing...';
    case 'verifying':
      return 'Verifying...';
    case 'cleaning_up':
      return 'Cleaning up...';
    case 'completed':
      return 'Completed';
    default:
      return '';
  }
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

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 5px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(156, 163, 175, 0.4);
  border-radius: 9999px;
}
.dark .custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(71, 85, 105, 0.5);
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(156, 163, 175, 0.7);
}

/* ==========================================================================
   Apple iOS / macOS Fluid Physics Animations
   Uses cubic-bezier(0.32, 0.72, 0, 1) iOS spring easing and origin-bottom-right
   ========================================================================== */

/* Desktop Drawer (Card) Transitions */
.ios-drawer-enter-active {
  transition:
    transform 0.36s cubic-bezier(0.32, 0.72, 0, 1),
    opacity 0.26s cubic-bezier(0.32, 0.72, 0, 1),
    filter 0.3s cubic-bezier(0.32, 0.72, 0, 1);
  will-change: transform, opacity, filter;
}

.ios-drawer-leave-active {
  position: absolute;
  bottom: 0;
  right: 0;
  pointer-events: none;
  transition:
    transform 0.26s cubic-bezier(0.32, 0.72, 0, 1),
    opacity 0.2s cubic-bezier(0.32, 0.72, 0, 1),
    filter 0.22s cubic-bezier(0.32, 0.72, 0, 1);
  will-change: transform, opacity, filter;
}

.ios-drawer-enter-from {
  opacity: 0;
  transform: scale(0.65) translateY(20px);
  filter: blur(10px);
}

.ios-drawer-leave-to {
  opacity: 0;
  transform: scale(0.65) translateY(20px);
  filter: blur(10px);
}

/* Desktop Floating Pill Transitions */
.ios-pill-enter-active {
  transition:
    transform 0.32s cubic-bezier(0.32, 0.72, 0, 1) 0.06s,
    opacity 0.24s cubic-bezier(0.32, 0.72, 0, 1) 0.06s,
    filter 0.26s cubic-bezier(0.32, 0.72, 0, 1) 0.06s;
  will-change: transform, opacity, filter;
}

.ios-pill-leave-active {
  position: absolute;
  bottom: 0;
  right: 0;
  pointer-events: none;
  transition:
    transform 0.18s cubic-bezier(0.32, 0.72, 0, 1),
    opacity 0.14s cubic-bezier(0.32, 0.72, 0, 1),
    filter 0.16s cubic-bezier(0.32, 0.72, 0, 1);
  will-change: transform, opacity, filter;
}

.ios-pill-enter-from {
  opacity: 0;
  transform: scale(0.75);
  filter: blur(6px);
}

.ios-pill-leave-to {
  opacity: 0;
  transform: scale(0.75);
  filter: blur(6px);
}

/* Mobile Bottom Sheet */
.ios-mobile-sheet-enter-active {
  transition: transform 0.38s cubic-bezier(0.32, 0.72, 0, 1), opacity 0.3s ease;
  will-change: transform, opacity;
}

.ios-mobile-sheet-leave-active {
  transition: transform 0.26s cubic-bezier(0.32, 0.72, 0, 1), opacity 0.22s ease;
  will-change: transform, opacity;
}

.ios-mobile-sheet-enter-from,
.ios-mobile-sheet-leave-to {
  transform: translateY(100%);
  opacity: 0.8;
}

/* Backdrop Fade */
.ios-fade-enter-active,
.ios-fade-leave-active {
  transition: opacity 0.22s cubic-bezier(0.32, 0.72, 0, 1);
}

.ios-fade-enter-from,
.ios-fade-leave-to {
  opacity: 0;
}
</style>
