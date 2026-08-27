<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="isOpen = false"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-3xl w-full flex flex-col shadow-2xl overflow-hidden max-h-[90vh]"
      @click.stop
    >
      <!-- Header -->
      <div class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0">
            <FbIcon name="settings" size="18px" />
          </div>
          <div>
            <h3 class="text-sm font-bold text-gray-900 dark:text-white">Settings & Preferences</h3>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">Configure application preferences, transfers, security, and storage paths.</p>
          </div>
        </div>

        <button
          @click="isOpen = false"
          class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        >
          ✕
        </button>
      </div>

      <!-- Navigation Tabs (6 Categories Matching Architecture Plan) -->
      <div class="flex border-b border-gray-200 dark:border-slate-800 px-6 bg-white dark:bg-[#0b0f19] text-xs font-semibold gap-6 overflow-x-auto">
        <button
          @click="activeTab = 'general'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'general'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="home" size="14px" />
          <span>General</span>
        </button>

        <button
          @click="activeTab = 'file_manager'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'file_manager'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="folder" size="14px" />
          <span>File Manager</span>
        </button>

        <button
          @click="activeTab = 'transfers'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'transfers'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="upload" size="14px" />
          <span>Transfers</span>
        </button>

        <button
          @click="activeTab = 'connections'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'connections'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="settings" size="14px" />
          <span>Connections</span>
        </button>

        <button
          @click="activeTab = 'security'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'security'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="shield" size="14px" />
          <span>Security</span>
        </button>

        <button
          @click="activeTab = 'advanced'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'advanced'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="clock" size="14px" />
          <span>Advanced & Logs</span>
        </button>
      </div>

      <!-- Tab Content Area -->
      <div class="p-6 overflow-y-auto flex-1 space-y-4 bg-white dark:bg-[#0b0f19]">
        <!-- 1. GENERAL -->
        <div v-if="activeTab === 'general'" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Default File View</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Choose between grid mosaic cards or compact list view.</p>
            </div>
            <select
              v-model="form.general.default_view"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold"
            >
              <option value="grid">Grid Mosaic</option>
              <option value="list">Details List</option>
            </select>
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Show Hidden & Dotfiles by Default</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Display hidden files (starting with '.') in explorer listings.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.general.show_hidden_default"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Confirm Destructive Actions</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Prompt confirmation dialog before deleting files or folders.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.general.confirm_destructive"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- 2. FILE MANAGER -->
        <div v-if="activeTab === 'file_manager'" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Default Workspace Layout</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Start in Dual-Pane side-by-side mode or Single Pane mode.</p>
            </div>
            <select
              v-model="form.file_manager.default_layout"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold"
            >
              <option value="split">Dual Pane (Split)</option>
              <option value="single">Single Pane</option>
            </select>
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">File List Density</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Row height and information density in List view.</p>
            </div>
            <div class="flex items-center space-x-1 bg-white dark:bg-slate-800 p-1 rounded-xl border border-gray-200 dark:border-slate-700">
              <button
                type="button"
                @click="uiStore.setListDensity('comfortable')"
                :class="[
                  'px-2.5 py-1 rounded-lg text-xs font-semibold transition cursor-pointer',
                  uiStore.listDensity === 'comfortable' ? 'bg-blue-600 text-white shadow-xs' : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                ]"
              >
                Comfortable
              </button>
              <button
                type="button"
                @click="uiStore.setListDensity('compact')"
                :class="[
                  'px-2.5 py-1 rounded-lg text-xs font-semibold transition cursor-pointer',
                  uiStore.listDensity === 'compact' ? 'bg-blue-600 text-white shadow-xs' : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                ]"
              >
                Compact
              </button>
              <button
                type="button"
                @click="uiStore.setListDensity('dense')"
                :class="[
                  'px-2.5 py-1 rounded-lg text-xs font-semibold transition cursor-pointer',
                  uiStore.listDensity === 'dense' ? 'bg-blue-600 text-white shadow-xs' : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                ]"
              >
                Dense
              </button>
            </div>
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Remember Last Directories</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Automatically restore the last opened folder path upon reconnection.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.file_manager.remember_last_directories"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- 3. TRANSFERS -->
        <div v-if="activeTab === 'transfers'" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Maximum Concurrent Transfers</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Number of simultaneous background file transfer streams (1–32).</p>
            </div>
            <input
              type="number"
              min="1"
              max="32"
              v-model.number="form.transfers.max_concurrent_transfers"
              class="w-20 bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-mono text-center"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Retry Attempts on Failure</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Exponential backoff retry count before marking a transfer failed.</p>
            </div>
            <input
              type="number"
              min="0"
              max="10"
              v-model.number="form.transfers.retry_attempts"
              class="w-20 bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-mono text-center"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Show Transfer Notifications</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Display toast notifications when transfer tasks finish or fail.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.transfers.show_notifications"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- 4. CONNECTIONS -->
        <div v-if="activeTab === 'connections'" class="space-y-4">
          <div class="bg-gray-50 dark:bg-slate-900/60 p-4 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <div class="flex items-center justify-between">
              <label class="block text-xs font-bold text-gray-900 dark:text-white">Default Local Storage Root Path</label>
              <span class="text-[10px] text-blue-600 dark:text-blue-400 font-mono font-semibold uppercase bg-blue-50 dark:bg-blue-900/40 px-2 py-0.5 rounded">Active Root</span>
            </div>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              Root directory mounted for the "Local" storage connection.
            </p>
            <input
              v-model="form.connections.default_local_root"
              type="text"
              placeholder="/home/user/storage or ./storage"
              class="w-full bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3.5 py-2.5 text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:border-blue-500 shadow-inner"
              required
            />
          </div>

          <div class="bg-gray-50 dark:bg-slate-900/60 p-4 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <label class="block text-xs font-bold text-gray-900 dark:text-white">Temporary & Staging Directory</label>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              Directory used for multipart uploads, streaming decompression, and temporary files.
            </p>
            <input
              v-model="form.connections.temp_dir"
              type="text"
              placeholder="./storage/temp or /tmp/aerofs"
              class="w-full bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3.5 py-2.5 text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:border-blue-500 shadow-inner"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Connection Timeout (Seconds)</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Network timeout before remote FTP/SFTP/S3 requests abort.</p>
            </div>
            <input
              type="number"
              min="10"
              max="300"
              v-model.number="form.connections.connection_timeout_secs"
              class="w-20 bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-mono text-center"
            />
          </div>
        </div>

        <!-- 5. SECURITY -->
        <div v-if="activeTab === 'security'" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Allow Symlinks Outside Root</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">When enabled, symlinks pointing outside the local root directory can be traversed.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.security.allow_symlinks_outside_root"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Global Read-Only Mode</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Lock the server to read-only; blocks all file writes, deletes, and uploads.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.security.read_only_default"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- 6. ADVANCED & AUDIT LOGS -->
        <div v-if="activeTab === 'advanced'" class="space-y-4">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Logging Level</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Log granularity for tracing operations and errors.</p>
            </div>
            <select
              v-model="form.advanced.log_level"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold font-mono"
            >
              <option value="info">INFO</option>
              <option value="debug">DEBUG</option>
              <option value="trace">TRACE</option>
            </select>
          </div>

          <!-- Audit Trail Section -->
          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <span class="text-xs font-bold text-gray-900 dark:text-white">Security & Access Audit Trail</span>
              <button
                @click="fetchAuditLogs"
                class="px-2.5 py-1 bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 rounded-lg text-[11px] font-semibold text-gray-700 dark:text-slate-300 flex items-center space-x-1"
              >
                <span>↻ Refresh Logs</span>
              </button>
            </div>

            <div class="border border-gray-200 dark:border-slate-800 rounded-2xl overflow-hidden max-h-56 overflow-y-auto font-mono text-[11px]">
              <table class="w-full text-left border-collapse">
                <thead class="bg-gray-50 dark:bg-slate-900 text-gray-500 dark:text-slate-400 text-[10px] uppercase font-bold border-b border-gray-200 dark:border-slate-800">
                  <tr>
                    <th class="py-2 px-3">Time</th>
                    <th class="py-2 px-2">Action</th>
                    <th class="py-2 px-2">Path / Target</th>
                    <th class="py-2 px-2">Status</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-gray-100 dark:divide-slate-800/60">
                  <tr v-for="log in auditLogs" :key="log.id" class="hover:bg-gray-50/50 dark:hover:bg-slate-900/40 text-gray-700 dark:text-slate-300">
                    <td class="py-2 px-3 text-gray-400 text-[10px] truncate max-w-[120px]">{{ formatTime(log.created_at) }}</td>
                    <td class="py-2 px-2 font-bold">{{ log.action }}</td>
                    <td class="py-2 px-2 truncate max-w-[200px]" :title="log.path || log.details">{{ log.path || log.details || '—' }}</td>
                    <td class="py-2 px-2">
                      <span
                        :class="[
                          'px-2 py-0.5 rounded text-[10px] font-semibold',
                          log.status === 'success' ? 'bg-emerald-100 dark:bg-emerald-950 text-emerald-700 dark:text-emerald-300' : 'bg-red-100 dark:bg-red-950 text-red-700 dark:text-red-300'
                        ]"
                      >
                        {{ log.status }}
                      </span>
                    </td>
                  </tr>
                  <tr v-if="auditLogs.length === 0">
                    <td colspan="4" class="py-6 text-center text-gray-400">No audit logs recorded yet.</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Buttons -->
      <div class="h-16 bg-gray-50 dark:bg-[#090d16] border-t border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <button
          type="button"
          @click="isOpen = false"
          class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium cursor-pointer"
        >
          Cancel
        </button>

        <button
          type="button"
          :disabled="saving"
          @click="handleSaveSettings"
          class="px-5 py-2.5 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded-xl transition shadow-xs cursor-pointer flex items-center space-x-1.5 disabled:opacity-50"
        >
          <span v-if="saving" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-white border-t-transparent"></span>
          <span>{{ saving ? 'Saving Changes...' : 'Save & Apply Settings' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();

const isOpen = ref(props.modelValue);
const activeTab = ref<'general' | 'file_manager' | 'transfers' | 'connections' | 'security' | 'advanced'>('general');
const saving = ref(false);
const auditLogs = ref<any[]>([]);

const form = ref({
  general: {
    language: 'en',
    theme: 'dark',
    default_view: 'grid',
    default_sort: 'name',
    sort_direction: 'asc',
    show_hidden_default: false,
    confirm_destructive: true,
  },
  file_manager: {
    default_layout: 'split',
    show_breadcrumbs: true,
    show_file_size: true,
    show_permissions: true,
    remember_last_directories: true,
  },
  transfers: {
    max_concurrent_transfers: 4,
    retry_attempts: 3,
    auto_retry: true,
    show_notifications: true,
  },
  connections: {
    connection_timeout_secs: 60,
    health_check_interval_secs: 30,
    auto_reconnect: true,
    default_local_root: './storage',
    temp_dir: './storage/temp',
  },
  security: {
    allow_symlinks_outside_root: false,
    confirm_permanent_delete: true,
    read_only_default: false,
    session_timeout_secs: 86400,
  },
  advanced: {
    log_level: 'info',
    enable_telemetry: true,
    enable_tracing: true,
    directory_cache_ttl_secs: 0,
  },
});

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      fetchSettings();
      fetchAuditLogs();
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function fetchSettings() {
  try {
    const resp = await apiClient.get('/settings');
    const data = resp.data;
    if (data.settings) {
      form.value = {
        general: { ...form.value.general, ...data.settings.general },
        file_manager: { ...form.value.file_manager, ...data.settings.file_manager },
        transfers: { ...form.value.transfers, ...data.settings.transfers },
        connections: { ...form.value.connections, ...data.settings.connections },
        security: { ...form.value.security, ...data.settings.security },
        advanced: { ...form.value.advanced, ...data.settings.advanced },
      };
    } else {
      form.value.connections.default_local_root = data.local_root || form.value.connections.default_local_root;
      form.value.connections.temp_dir = data.temp_dir || form.value.connections.temp_dir;
      form.value.security.allow_symlinks_outside_root = data.allow_symlinks ?? false;
      form.value.general.show_hidden_default = data.show_hidden_default ?? false;
      form.value.security.read_only_default = data.read_only_default ?? false;
    }
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to load settings', 'error');
  }
}

async function fetchAuditLogs() {
  try {
    const resp = await apiClient.get('/audit-logs');
    auditLogs.value = resp.data;
  } catch {}
}

async function handleSaveSettings() {
  saving.value = true;
  try {
    const resp = await apiClient.put('/settings', { settings: form.value });
    uiStore.showToast(resp.data.message || 'Settings updated successfully!', 'success');
    isOpen.value = false;

    // Apply show_hidden and view_mode to workspace panels
    workspaceStore.leftPanel.showHidden = form.value.general.show_hidden_default;
    workspaceStore.rightPanel.showHidden = form.value.general.show_hidden_default;
    if (form.value.general.default_view === 'grid' || form.value.general.default_view === 'list') {
      workspaceStore.leftPanel.viewMode = form.value.general.default_view;
      workspaceStore.rightPanel.viewMode = form.value.general.default_view;
    }
    workspaceStore.saveState();

    // Refresh workspace file list with new settings
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to save settings', 'error');
  } finally {
    saving.value = false;
  }
}

function formatTime(dateStr?: string): string {
  if (!dateStr) return '—';
  const d = new Date(dateStr);
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}
</script>
