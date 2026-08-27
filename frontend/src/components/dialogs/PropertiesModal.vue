<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="isOpen = false"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-lg w-full flex flex-col shadow-2xl overflow-hidden max-h-[85vh]"
      @click.stop
    >
      <!-- Modal Header -->
      <div class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <div class="flex items-center space-x-3 truncate">
          <div
            :class="[
              'w-9 h-9 rounded-xl flex items-center justify-center shrink-0',
              meta?.kind === 'directory'
                ? 'bg-amber-500/10 text-amber-500'
                : 'bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400'
            ]"
          >
            <FbIcon :name="meta?.kind === 'directory' ? 'folder' : 'file'" size="18px" />
          </div>
          <div class="truncate">
            <h3 class="text-sm font-bold text-gray-900 dark:text-white truncate">
              {{ meta?.name || 'File Properties' }}
            </h3>
            <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate">
              {{ meta?.path || path }}
            </p>
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
      <div class="flex border-b border-gray-200 dark:border-slate-800 px-6 bg-white dark:bg-[#0b0f19] text-xs font-semibold gap-6">
        <button
          @click="activeTab = 'general'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-2',
            activeTab === 'general'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="info" size="14px" />
          <span>General Info</span>
        </button>

        <button
          @click="activeTab = 'permissions'"
          :class="[
            'py-3 border-b-2 transition cursor-pointer flex items-center space-x-2',
            activeTab === 'permissions'
              ? 'border-blue-600 text-blue-600 dark:text-blue-400'
              : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
          ]"
        >
          <FbIcon name="settings" size="14px" />
          <span>Permissions (CHMOD)</span>
        </button>
      </div>

      <!-- Content Area -->
      <div class="p-6 overflow-y-auto flex-1 space-y-4 bg-white dark:bg-[#0b0f19]">
        <!-- Loading State -->
        <div v-if="loading" class="py-12 flex flex-col items-center justify-center space-y-2 text-gray-400">
          <div class="animate-spin rounded-full h-6 w-6 border-2 border-blue-600 border-t-transparent"></div>
          <span>Loading properties...</span>
        </div>

        <template v-else-if="meta">
          <!-- TAB 1: General Info -->
          <div v-if="activeTab === 'general'" class="space-y-3 font-sans text-xs">
            <div class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-2.5">
              <!-- Name & Type -->
              <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
                <span class="text-gray-500 dark:text-slate-400">Item Type:</span>
                <span class="font-bold text-gray-900 dark:text-white capitalize">
                  {{ meta.kind }} {{ meta.symlink_target ? '(Symlink)' : '' }}
                </span>
              </div>

              <!-- MIME Type -->
              <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
                <span class="text-gray-500 dark:text-slate-400">MIME Type:</span>
                <span class="font-mono text-gray-800 dark:text-slate-200">
                  {{ meta.mime_type || (meta.kind === 'directory' ? 'inode/directory' : 'application/octet-stream') }}
                </span>
              </div>

              <!-- File Size -->
              <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
                <span class="text-gray-500 dark:text-slate-400">File Size:</span>
                <span class="font-mono text-gray-800 dark:text-slate-200">
                  {{ formatSize(meta.size) }} ({{ meta.size.toLocaleString() }} bytes)
                </span>
              </div>

              <!-- Location / Path -->
              <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
                <span class="text-gray-500 dark:text-slate-400">Full Path:</span>
                <div class="flex items-center space-x-1.5 truncate max-w-[260px]">
                  <span class="font-mono text-gray-800 dark:text-slate-200 truncate" :title="meta.path">{{ meta.path }}</span>
                  <button
                    @click="copyPath(meta.path)"
                    class="text-blue-600 hover:text-blue-700 dark:text-blue-400 cursor-pointer p-0.5"
                    title="Copy full path"
                  >
                    📋
                  </button>
                </div>
              </div>

              <!-- Modified Date -->
              <div class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
                <span class="text-gray-500 dark:text-slate-400">Last Modified:</span>
                <span class="font-mono text-gray-800 dark:text-slate-200">{{ formatDate(meta.modified_at) }}</span>
              </div>

              <!-- Created Date -->
              <div v-if="meta.created_at" class="flex justify-between py-1 border-b border-gray-100 dark:border-slate-800">
                <span class="text-gray-500 dark:text-slate-400">Created:</span>
                <span class="font-mono text-gray-800 dark:text-slate-200">{{ formatDate(meta.created_at) }}</span>
              </div>

              <!-- ETag / Checksum -->
              <div class="flex justify-between py-1">
                <span class="text-gray-500 dark:text-slate-400">ETag / Concurrency:</span>
                <span class="font-mono text-gray-400 dark:text-slate-500 text-[11px] truncate max-w-[240px]">{{ meta.etag }}</span>
              </div>
            </div>
          </div>

          <!-- TAB 2: Permissions (CHMOD Matrix) -->
          <div v-if="activeTab === 'permissions'" class="space-y-4 font-sans text-xs">
            <div class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 space-y-4">
              <!-- Matrix Header -->
              <div class="flex items-center justify-between">
                <div>
                  <h4 class="font-bold text-gray-900 dark:text-white">Unix File Permissions</h4>
                  <p class="text-[11px] text-gray-400 dark:text-slate-500">Read (r), Write (w), and Execute (x) flags.</p>
                </div>

                <!-- Octal Box -->
                <div class="flex items-center space-x-1.5">
                  <span class="text-gray-500 dark:text-slate-400 font-mono">Octal:</span>
                  <input
                    v-model="octalMode"
                    @input="onOctalInput"
                    type="text"
                    maxlength="4"
                    class="w-16 bg-white dark:bg-slate-950 border border-gray-200 dark:border-slate-700 rounded-xl px-2 py-1 text-center font-mono font-bold text-blue-600 dark:text-blue-400 focus:outline-none focus:border-blue-500 shadow-inner"
                  />
                </div>
              </div>

              <!-- Interactive 3x3 Permissions Table -->
              <div class="border border-gray-200 dark:border-slate-800 rounded-2xl overflow-hidden bg-white dark:bg-slate-950">
                <table class="w-full text-center border-collapse text-xs">
                  <thead class="bg-gray-50 dark:bg-slate-900/80 text-gray-500 dark:text-slate-400 text-[11px] font-semibold border-b border-gray-200 dark:border-slate-800">
                    <tr>
                      <th class="py-2.5 px-3 text-left">Scope</th>
                      <th class="py-2.5 px-2">Read (4)</th>
                      <th class="py-2.5 px-2">Write (2)</th>
                      <th class="py-2.5 px-2">Execute (1)</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-gray-100 dark:divide-slate-800">
                    <!-- Owner / User -->
                    <tr class="hover:bg-gray-50/50 dark:hover:bg-slate-900/40">
                      <td class="py-2.5 px-3 text-left font-bold text-gray-900 dark:text-white">Owner (User)</td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.user.r" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.user.w" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.user.x" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                    </tr>
                    <!-- Group -->
                    <tr class="hover:bg-gray-50/50 dark:hover:bg-slate-900/40">
                      <td class="py-2.5 px-3 text-left font-bold text-gray-900 dark:text-white">Group</td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.group.r" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.group.w" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.group.x" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                    </tr>
                    <!-- Others / Public -->
                    <tr class="hover:bg-gray-50/50 dark:hover:bg-slate-900/40">
                      <td class="py-2.5 px-3 text-left font-bold text-gray-900 dark:text-white">Others (Public)</td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.other.r" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.other.w" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                      <td class="py-2.5 px-2"><input type="checkbox" v-model="permState.other.x" @change="recalcOctal" class="rounded text-blue-600 cursor-pointer" /></td>
                    </tr>
                  </tbody>
                </table>
              </div>

              <!-- Recursive Toggle (If Directory) -->
              <div v-if="meta.kind === 'directory'" class="flex items-center space-x-2 pt-1">
                <input type="checkbox" id="recCheck" v-model="applyRecursive" class="rounded text-blue-600 cursor-pointer" />
                <label for="recCheck" class="text-gray-700 dark:text-slate-300 font-medium cursor-pointer">
                  Apply recursively to all enclosed files and subfolders
                </label>
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- Modal Footer -->
      <div class="h-16 bg-gray-50 dark:bg-[#090d16] border-t border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <button
          type="button"
          @click="isOpen = false"
          class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium cursor-pointer"
        >
          Close
        </button>

        <div class="flex items-center space-x-2">
          <button
            v-if="activeTab === 'permissions'"
            type="button"
            :disabled="savingPerms"
            @click="handleSavePermissions"
            class="px-5 py-2.5 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded-xl transition shadow-xs cursor-pointer flex items-center space-x-1.5 disabled:opacity-50"
          >
            <span v-if="savingPerms" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-white border-t-transparent"></span>
            <span>{{ savingPerms ? 'Applying...' : 'Save Permissions' }}</span>
          </button>
        </div>
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
  connectionId: string;
  path: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();

const isOpen = ref(props.modelValue);
const activeTab = ref<'general' | 'permissions'>('general');
const meta = ref<any | null>(null);
const loading = ref(false);
const savingPerms = ref(false);
const applyRecursive = ref(false);

const octalMode = ref('0755');
const permState = ref({
  user: { r: true, w: true, x: true },
  group: { r: true, w: false, x: true },
  other: { r: true, w: false, x: true },
});

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val && props.path) {
      fetchMetadata();
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function fetchMetadata() {
  loading.value = true;
  try {
    const resp = await apiClient.get(
      `/connections/${props.connectionId}/files/metadata?path=${encodeURIComponent(props.path)}`
    );
    meta.value = resp.data;

    // Parse permissions if available
    if (meta.value.permissions) {
      parsePermissionsString(meta.value.permissions);
    } else {
      octalMode.value = meta.value.kind === 'directory' ? '0755' : '0644';
      parseOctal(octalMode.value);
    }
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to fetch file metadata', 'error');
  } finally {
    loading.value = false;
  }
}

function parsePermissionsString(p: string) {
  // e.g. -rwxr-xr-x
  const clean = p.length === 10 ? p.substring(1) : p;
  if (clean.length === 9) {
    permState.value = {
      user: { r: clean[0] === 'r', w: clean[1] === 'w', x: clean[2] === 'x' },
      group: { r: clean[3] === 'r', w: clean[4] === 'w', x: clean[5] === 'x' },
      other: { r: clean[6] === 'r', w: clean[7] === 'w', x: clean[8] === 'x' },
    };
    recalcOctal();
  }
}

function parseOctal(oct: string) {
  const clean = oct.replace(/^0+/, '').padStart(3, '0');
  const u = parseInt(clean[0] || '0', 10);
  const g = parseInt(clean[1] || '0', 10);
  const o = parseInt(clean[2] || '0', 10);

  permState.value = {
    user: { r: (u & 4) !== 0, w: (u & 2) !== 0, x: (u & 1) !== 0 },
    group: { r: (g & 4) !== 0, w: (g & 2) !== 0, x: (g & 1) !== 0 },
    other: { r: (o & 4) !== 0, w: (o & 2) !== 0, x: (o & 1) !== 0 },
  };
}

function onOctalInput() {
  parseOctal(octalMode.value);
}

function recalcOctal() {
  let u = (permState.value.user.r ? 4 : 0) + (permState.value.user.w ? 2 : 0) + (permState.value.user.x ? 1 : 0);
  let g = (permState.value.group.r ? 4 : 0) + (permState.value.group.w ? 2 : 0) + (permState.value.group.x ? 1 : 0);
  let o = (permState.value.other.r ? 4 : 0) + (permState.value.other.w ? 2 : 0) + (permState.value.other.x ? 1 : 0);

  octalMode.value = `0${u}${g}${o}`;
}

async function handleSavePermissions() {
  savingPerms.value = true;
  try {
    const modeInt = parseInt(octalMode.value, 8);
    await apiClient.post(`/connections/${props.connectionId}/files/chmod`, {
      path: props.path,
      mode: modeInt,
      recursive: applyRecursive.value,
    });

    uiStore.showToast('Permissions updated successfully!', 'success');
    await fetchMetadata();
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to update permissions', 'error');
  } finally {
    savingPerms.value = false;
  }
}

function copyPath(p: string) {
  navigator.clipboard.writeText(p);
  uiStore.showToast('Path copied to clipboard', 'success');
}

function formatSize(bytes?: number): string {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return '—';
  const d = new Date(dateStr);
  return d.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}
</script>
