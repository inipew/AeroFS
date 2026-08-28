<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs"
      @click="isOpen = false"
    >
      <div class="modal-card bg-slate-900 border border-slate-700 rounded-2xl max-w-3xl w-full p-6 shadow-2xl flex flex-col max-h-[85vh] overflow-hidden" @click.stop>
      <!-- Header -->
      <div class="flex items-center justify-between pb-3 border-b border-slate-800">
        <div class="flex items-center space-x-2">
          <span class="text-base">🛡️</span>
          <div>
            <h3 class="text-sm font-bold text-white">System Audit Logs</h3>
            <p class="text-[10px] text-slate-400">Security event & user activity tracking</p>
          </div>
        </div>

        <div class="flex items-center space-x-2">
          <button
            @click="fetchLogs"
            class="px-2.5 py-1 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded text-xs transition"
          >
            Refresh
          </button>
          <button
            @click="isOpen = false"
            class="text-slate-400 hover:text-white px-2 py-1 text-sm"
          >
            &times;
          </button>
        </div>
      </div>

      <!-- Logs Table -->
      <div class="flex-1 overflow-y-auto mt-3 bg-slate-950 rounded-xl border border-slate-800">
        <table class="w-full text-left border-collapse text-[11px]">
          <thead class="bg-slate-900 sticky top-0 border-b border-slate-800 text-[10px] text-slate-400 uppercase font-semibold">
            <tr>
              <th class="py-2 px-3 w-40">Timestamp</th>
              <th class="py-2 px-3 w-24">User</th>
              <th class="py-2 px-3 w-36">Action</th>
              <th class="py-2 px-3">Details</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-900 text-slate-300 font-mono">
            <tr v-if="loading">
              <td colspan="4" class="py-8 text-center text-slate-500 font-sans">
                Loading audit logs...
              </td>
            </tr>
            <tr v-else-if="logs.length === 0">
              <td colspan="4" class="py-8 text-center text-slate-500 font-sans">
                No audit logs recorded yet
              </td>
            </tr>
            <tr v-for="log in logs" :key="log.id" class="hover:bg-slate-900/60 transition">
              <td class="py-2 px-3 text-slate-500 text-[10px]">{{ formatDate(log.created_at) }}</td>
              <td class="py-2 px-3 text-indigo-300 font-semibold">{{ log.username }}</td>
              <td class="py-2 px-3">
                <span class="px-1.5 py-0.2 rounded bg-indigo-500/10 text-indigo-300 border border-indigo-500/20 text-[10px]">
                  {{ log.action }}
                </span>
              </td>
              <td class="py-2 px-3 text-slate-400 truncate max-w-xs">{{ log.details || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { apiClient } from '../../api/client';

export interface AuditLogEntry {
  id: string;
  user_id?: string;
  username: string;
  action: string;
  ip_address?: string;
  details?: string;
  created_at: string;
}

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const isOpen = ref(props.modelValue);
const logs = ref<AuditLogEntry[]>([]);
const loading = ref(false);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      fetchLogs();
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function fetchLogs() {
  loading.value = true;
  try {
    const resp = await apiClient.get<AuditLogEntry[]>('/audit-logs');
    logs.value = resp.data;
  } catch {
    logs.value = [];
  } finally {
    loading.value = false;
  }
}

function formatDate(dateStr: string): string {
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
