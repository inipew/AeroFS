<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs"
      @click="isOpen = false"
    >
      <div
        class="modal-card bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-3xl w-full flex flex-col shadow-2xl overflow-hidden max-h-[90vh]"
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
            <p class="text-[11px] text-gray-500 dark:text-slate-400">Configure appearance, workspace, shortcuts, and system administration.</p>
          </div>
        </div>

        <button
          @click="isOpen = false"
          class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        >
          ✕
        </button>
      </div>

      <!-- Navigation Tabs (Grouped: User Preferences vs System Administration) -->
      <div class="flex border-b border-gray-200 dark:border-slate-800 px-6 bg-white dark:bg-[#0b0f19] text-xs font-semibold gap-6 overflow-x-auto no-scrollbar">
        <!-- USER TABS -->
        <button
          @click="activeTab = 'appearance'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'appearance'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="home" size="14px" />
          <span>Appearance</span>
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
          @click="activeTab = 'shortcuts'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
            activeTab === 'shortcuts'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="rename" size="14px" />
          <span>Shortcuts</span>
        </button>

        <!-- ADMIN TABS -->
        <template v-if="authStore.isAdmin">
          <div class="h-4 w-px bg-gray-200 dark:bg-slate-800 my-auto shrink-0"></div>

          <button
            @click="activeTab = 'admin_storage'"
            :class="[
              'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
              activeTab === 'admin_storage'
                ? 'border-blue-600 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
            ]"
          >
            <FbIcon name="settings" size="14px" />
            <span>Server Storage</span>
          </button>

          <button
            @click="activeTab = 'admin_security'"
            :class="[
              'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
              activeTab === 'admin_security'
                ? 'border-blue-600 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
            ]"
          >
            <FbIcon name="shield" size="14px" />
            <span>Security & Policy</span>
          </button>

          <button
            @click="activeTab = 'admin_logs'"
            :class="[
              'py-3 border-b-2 transition cursor-pointer flex items-center space-x-1.5 shrink-0',
              activeTab === 'admin_logs'
                ? 'border-blue-600 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
            ]"
          >
            <FbIcon name="clock" size="14px" />
            <span>Audit Logs</span>
          </button>
        </template>
      </div>

      <!-- Tab Content Area -->
      <div class="p-6 overflow-y-auto flex-1 space-y-4 bg-white dark:bg-[#0b0f19]">
        <!-- 1. APPEARANCE (USER) -->
        <div v-if="activeTab === 'appearance'" class="space-y-3">
          <!-- Theme Selector -->
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Color Theme</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Choose between dark mode, light mode, or follow system theme.</p>
            </div>
            <select
              v-model="userPrefs.theme"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold"
            >
              <option value="system">System Default</option>
              <option value="dark">Dark Theme</option>
              <option value="light">Light Theme</option>
            </select>
          </div>

          <!-- List Density -->
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">List & Card Density</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Control the padding and row spacing in file lists.</p>
            </div>
            <select
              v-model="userPrefs.list_density"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold"
            >
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
              <option value="dense">Dense</option>
            </select>
          </div>

          <!-- Default File View -->
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Default View Mode</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Default layout when opening folders.</p>
            </div>
            <select
              v-model="userPrefs.default_view"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold"
            >
              <option value="grid">Grid Mosaic Cards</option>
              <option value="list">Details List</option>
            </select>
          </div>

          <!-- Default Workspace Layout -->
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Default Workspace Layout</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Choose between Single Pane or Dual-Pane Split mode on first launch.</p>
            </div>
            <select
              v-model="userPrefs.default_layout"
              class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 text-xs font-semibold"
            >
              <option value="single">Single Pane</option>
              <option value="split">Dual-Pane Split (Commander)</option>
            </select>
          </div>
        </div>

        <!-- 2. FILE MANAGER (USER) -->
        <div v-if="activeTab === 'file_manager'" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Show Hidden & Dotfiles</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Display hidden files (starting with '.') in explorer listings.</p>
            </div>
            <input
              type="checkbox"
              v-model="userPrefs.show_hidden"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Remember Last Directories</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Restore last visited folders across sessions.</p>
            </div>
            <input
              type="checkbox"
              v-model="userPrefs.remember_last_dir"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- 3. TRANSFERS (USER) -->
        <div v-if="activeTab === 'transfers'" class="space-y-3">
          <div class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <div class="flex items-center justify-between">
              <div>
                <p class="font-bold text-gray-900 dark:text-white text-xs">Maximum Parallel Workers</p>
                <p class="text-[11px] text-gray-500 dark:text-slate-400">Number of simultaneous chunked uploads or downloads.</p>
              </div>
              <input
                type="number"
                min="1"
                max="8"
                v-model.number="adminForm.transfers.max_concurrent_transfers"
                class="w-20 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-xs font-mono font-bold text-center"
              />
            </div>
          </div>
        </div>

        <!-- 4. KEYBOARD SHORTCUTS REFERENCE (USER) -->
        <div v-if="activeTab === 'shortcuts'" class="space-y-2">
          <div class="text-[11px] text-gray-500 dark:text-slate-400 px-1 mb-2">
            AeroFS is designed for power users with comprehensive keyboard control:
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Command Palette</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Ctrl + K</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Edit / Jump Path</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Ctrl + L</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Switch Dual Pane Focus</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Tab</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Swap Left & Right Panels</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Alt + S</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Copy Selected Items</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Ctrl + C</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Cut Selected Items</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Ctrl + X</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Paste Clipboard</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Ctrl + V</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Rename Item</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">F2</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Delete Item</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Delete</kbd>
            </div>

            <div class="p-3 bg-gray-50 dark:bg-slate-900/60 rounded-xl border border-gray-200 dark:border-slate-800 flex items-center justify-between">
              <span class="text-xs text-gray-700 dark:text-slate-300">Navigate to Parent</span>
              <kbd class="px-2 py-0.5 rounded bg-gray-200 dark:bg-slate-800 font-mono text-[10px] font-bold text-gray-600 dark:text-slate-300 shadow-2xs border border-gray-300 dark:border-slate-700">Alt + Up</kbd>
            </div>
          </div>
        </div>

        <!-- 5. SERVER STORAGE (ADMIN ONLY) -->
        <div v-if="activeTab === 'admin_storage' && authStore.isAdmin" class="space-y-3">
          <div class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <label class="block font-bold text-gray-900 dark:text-white text-xs">Default Local Storage Root</label>
            <input
              type="text"
              v-model="adminForm.connections.default_local_root"
              class="w-full bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-2 text-xs font-mono text-gray-800 dark:text-slate-100"
              placeholder="./storage"
            />
            <p class="text-[11px] text-gray-400">Absolute or relative filesystem path exposed as primary local storage root.</p>
          </div>

          <div class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2">
            <label class="block font-bold text-gray-900 dark:text-white text-xs">Temporary & Staging Directory</label>
            <input
              type="text"
              v-model="adminForm.connections.temp_dir"
              class="w-full bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-2 text-xs font-mono text-gray-800 dark:text-slate-100"
              placeholder="./storage/temp"
            />
            <p class="text-[11px] text-gray-400">Path used for streaming chunk uploads and archive extractions.</p>
          </div>
        </div>

        <!-- 6. SECURITY & POLICY (ADMIN ONLY) -->
        <div v-if="activeTab === 'admin_security' && authStore.isAdmin" class="space-y-3">
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Allow Symlinks Outside Root</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">When disabled, symlinks pointing outside storage root are strictly rejected.</p>
            </div>
            <input
              type="checkbox"
              v-model="adminForm.security.allow_symlinks_outside_root"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>

          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800">
            <div>
              <p class="font-bold text-gray-900 dark:text-white text-xs">Read-Only Default Mode</p>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">Lock entire storage root to read-only for guest/non-admin users.</p>
            </div>
            <input
              type="checkbox"
              v-model="adminForm.security.read_only_default"
              class="h-5 w-5 rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </div>
        </div>

        <!-- 7. AUDIT LOGS (ADMIN ONLY) -->
        <div v-if="activeTab === 'admin_logs' && authStore.isAdmin" class="space-y-3">
          <div v-if="auditLogs.length === 0" class="text-center py-8 text-gray-400 text-xs">
            No system audit logs found.
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="(log, idx) in auditLogs"
              :key="idx"
              class="p-3 bg-gray-50 dark:bg-slate-900/40 rounded-xl border border-gray-200/60 dark:border-slate-800/60 text-[11px] flex items-center justify-between"
            >
              <div class="truncate flex-1 space-y-0.5">
                <p class="font-semibold text-gray-800 dark:text-slate-200 truncate">{{ log.action }}</p>
                <p class="text-gray-400 font-mono text-[10px] truncate">{{ log.user_id }} · {{ log.details }}</p>
              </div>
              <span class="text-[10px] text-gray-400 font-mono shrink-0 ml-3">{{ formatTime(log.timestamp) }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="h-16 bg-gray-50 dark:bg-[#090d16] border-t border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <button
          @click="isOpen = false"
          class="px-4 py-2 rounded-xl text-gray-500 dark:text-slate-400 hover:bg-gray-200/60 dark:hover:bg-slate-800 transition cursor-pointer font-semibold"
        >
          Cancel
        </button>

        <button
          @click="handleSaveSettings"
          :disabled="saving"
          class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-bold transition shadow-md shadow-blue-500/20 cursor-pointer flex items-center space-x-1.5 disabled:opacity-50"
        >
          <span v-if="saving" class="animate-spin text-xs">⏳</span>
          <span>Save Changes</span>
        </button>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useAuthStore } from '../../stores/authStore';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { usePreferencesStore } from '../../stores/preferencesStore';

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ (e: 'update:modelValue', val: boolean): void }>();

const authStore = useAuthStore();
const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();
const preferencesStore = usePreferencesStore();

const isOpen = ref(props.modelValue);
const activeTab = ref('appearance');
const saving = ref(false);
const auditLogs = ref<any[]>([]);

// User Preferences Reactive Model
const userPrefs = ref({ ...preferencesStore.preferences });

// Admin System Settings Model
const adminForm = ref({
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
});

watch(
  () => props.modelValue,
  async (val) => {
    isOpen.value = val;
    if (val) {
      userPrefs.value = { ...preferencesStore.preferences };
      if (authStore.isAdmin) {
        await fetchAdminSettings();
        await fetchAuditLogs();
      }
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

const fullSettings = ref<any>({});

async function fetchAdminSettings() {
  try {
    const resp = await apiClient.get('/settings');
    const data = resp.data;
    if (data.max_editable_size) {
      uiStore.setMaxEditableSize(data.max_editable_size);
    } else if (data.settings?.file_manager?.max_editable_size) {
      uiStore.setMaxEditableSize(data.settings.file_manager.max_editable_size);
    }
    if (data.settings) {
      fullSettings.value = data.settings;
      adminForm.value = {
        transfers: { ...adminForm.value.transfers, ...data.settings.transfers },
        connections: {
          ...adminForm.value.connections,
          ...data.settings.connections,
          default_local_root: data.local_root || data.settings.connections?.default_local_root || './storage',
          temp_dir: data.temp_dir || data.settings.connections?.temp_dir || './storage/temp',
        },
        security: { ...adminForm.value.security, ...data.settings.security },
      };
    }
  } catch (err: any) {
    console.warn('Failed to load admin settings', err);
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
    // 1. Save User Preferences to unified preferencesStore & API
    await preferencesStore.updatePreferences(userPrefs.value);

    // 2. If Admin, also save System Settings
    if (authStore.isAdmin) {
      const mergedSettings = {
        ...fullSettings.value,
        transfers: adminForm.value.transfers,
        connections: adminForm.value.connections,
        security: adminForm.value.security,
      };
      await apiClient.put('/settings', {
        settings: mergedSettings,
        local_root: adminForm.value.connections.default_local_root,
        temp_dir: adminForm.value.connections.temp_dir,
        allow_symlinks: adminForm.value.security.allow_symlinks_outside_root,
        read_only_default: adminForm.value.security.read_only_default,
      });
    }

    uiStore.showToast('Settings & preferences saved successfully!', 'success');
    isOpen.value = false;

    // 3. Refresh Workspace with updated preferences
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
