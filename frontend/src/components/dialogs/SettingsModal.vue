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
            <h3 class="text-sm font-bold text-gray-900 dark:text-white">Settings & Administration</h3>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">Configure root directories, storage paths, security, and audit logs.</p>
          </div>
        </div>

        <button
          @click="isOpen = false"
          class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        >
          ✕
        </button>
      </div>

      <!-- Navigation Tabs -->
      <div class="flex border-b border-gray-200 dark:border-slate-800 px-6 bg-white dark:bg-[#0b0f19] text-xs font-semibold gap-6 overflow-x-auto">
        <button
          @click="activeTab = 'storage'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-2 shrink-0',
            activeTab === 'storage'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="folder" size="14px" />
          <span>Storage & Root Path</span>
        </button>

        <button
          @click="activeTab = 'filesystem'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-2 shrink-0',
            activeTab === 'filesystem'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="settings" size="14px" />
          <span>Filesystem & Security</span>
        </button>

        <button
          @click="activeTab = 'audit'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-2 shrink-0',
            activeTab === 'audit'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="clock" size="14px" />
          <span>Audit Logs</span>
        </button>

        <button
          @click="activeTab = 'system'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-2 shrink-0',
            activeTab === 'system'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="home" size="14px" />
          <span>System Info</span>
        </button>
      </div>

      <!-- Tab Content Area -->
      <div class="p-6 overflow-y-auto flex-1 space-y-5 bg-white dark:bg-[#0b0f19]">
        <!-- TAB 1: Storage & Root Paths -->
        <div v-if="activeTab === 'storage'" class="space-y-4">
          <!-- Root Path Field -->
          <div class="bg-gray-50 dark:bg-slate-900/60 p-4 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <div class="flex items-center justify-between">
              <label class="block text-xs font-bold text-gray-900 dark:text-white">Default Local Storage Root Path</label>
              <span class="text-[10px] text-blue-600 dark:text-blue-400 font-mono font-semibold uppercase bg-blue-50 dark:bg-blue-900/40 px-2 py-0.5 rounded">Active Root</span>
            </div>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              The root directory mapped to the "Local" storage connection. Can be an absolute path (e.g. <code class="font-mono text-gray-800 dark:text-slate-200">/home/dhimas/storage</code>) or relative path.
            </p>
            <input
              v-model="form.local_root"
              type="text"
              placeholder="/var/www or ./storage"
              class="w-full bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3.5 py-2.5 text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:border-blue-500 shadow-inner"
              required
            />
          </div>

          <!-- Temp / Cache Directory Field -->
          <div class="bg-gray-50 dark:bg-slate-900/60 p-4 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <label class="block text-xs font-bold text-gray-900 dark:text-white">Temporary & Cache Directory</label>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">
              Directory used for multipart uploads, streaming decompression, and temporary archive staging.
            </p>
            <input
              v-model="form.temp_dir"
              type="text"
              placeholder="./storage/temp or /tmp/filemanager"
              class="w-full bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-3.5 py-2.5 text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:border-blue-500 shadow-inner"
            />
          </div>
        </div>

        <!-- TAB 2: Filesystem & Security Options -->
        <div v-if="activeTab === 'filesystem'" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Allow Symlinks Outside Root</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">When enabled, symlinks pointing outside the root directory can be followed.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.allow_symlinks"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Show Dotfiles & Hidden Items</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Display hidden files (starting with '.') in explorer listings by default.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.show_hidden_default"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Global Read-Only Mode</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Prevent all file creation, write, delete, and upload operations.</p>
            </div>
            <input
              type="checkbox"
              v-model="form.read_only_default"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- TAB 3: Audit Logs Tab (Real-Time Inspection) -->
        <div v-if="activeTab === 'audit'" class="space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold text-gray-900 dark:text-white">Security & Access Audit Trail</span>
            <button
              @click="fetchAuditLogs"
              class="px-2.5 py-1 bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 rounded-lg text-[11px] font-semibold text-gray-700 dark:text-slate-300 flex items-center space-x-1"
            >
              <span>↻ Refresh Logs</span>
            </button>
          </div>

          <div class="border border-gray-200 dark:border-slate-800 rounded-2xl overflow-hidden max-h-72 overflow-y-auto font-mono text-[11px]">
            <table class="w-full text-left border-collapse">
              <thead class="bg-gray-50 dark:bg-slate-900 text-gray-500 dark:text-slate-400 text-[10px] uppercase font-bold border-b border-gray-200 dark:border-slate-800">
                <tr>
                  <th class="py-2 px-3">Time</th>
                  <th class="py-2 px-2">Action</th>
                  <th class="py-2 px-2">Path / Details</th>
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
                  <td colspan="4" class="py-8 text-center text-gray-400">No audit logs recorded yet.</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- TAB 4: System Info -->
        <div v-if="activeTab === 'system'" class="space-y-3 font-mono text-xs">
          <div class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
              <span class="text-gray-500 dark:text-slate-400">Database Engine:</span>
              <span class="font-bold text-gray-800 dark:text-slate-200 truncate max-w-[300px]">{{ systemInfo.database_url }}</span>
            </div>
            <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
              <span class="text-gray-500 dark:text-slate-400">Max Upload Limit:</span>
              <span class="font-bold text-gray-800 dark:text-slate-200">{{ systemInfo.max_upload_mb }} MB</span>
            </div>
            <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
              <span class="text-gray-500 dark:text-slate-400">Runtime Protocol:</span>
              <span class="font-bold text-emerald-600 dark:text-emerald-400">HTTP/1.1 + WebSocket + TLS</span>
            </div>
            <div class="flex justify-between py-1">
              <span class="text-gray-500 dark:text-slate-400">Backend Core:</span>
              <span class="font-bold text-blue-600 dark:text-blue-400">Rust (Axum + Tokio + VFS)</span>
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
const activeTab = ref<'storage' | 'filesystem' | 'audit' | 'system'>('storage');
const saving = ref(false);
const auditLogs = ref<any[]>([]);

const form = ref({
  local_root: './storage',
  temp_dir: './storage/temp',
  allow_symlinks: false,
  show_hidden_default: false,
  read_only_default: false,
});

const systemInfo = ref({
  database_url: 'sqlite://./filemanager.db',
  max_upload_mb: 1024,
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
    form.value.local_root = data.local_root;
    form.value.temp_dir = data.temp_dir;
    form.value.allow_symlinks = data.allow_symlinks;
    form.value.show_hidden_default = data.show_hidden_default;
    form.value.read_only_default = data.read_only_default;

    systemInfo.value.database_url = data.database_url;
    systemInfo.value.max_upload_mb = data.max_upload_mb;
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
    const resp = await apiClient.put('/settings', form.value);
    uiStore.showToast(resp.data.message || 'Settings updated successfully!', 'success');
    isOpen.value = false;

    // Apply show_hidden to workspace panels
    workspaceStore.leftPanel.showHidden = form.value.show_hidden_default;
    workspaceStore.rightPanel.showHidden = form.value.show_hidden_default;
    workspaceStore.saveState();

    // Refresh workspace file list with new root and show_hidden settings
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
