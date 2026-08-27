<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="isOpen = false"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-2xl w-full flex flex-col shadow-2xl overflow-hidden max-h-[85vh]"
      @click.stop
    >
      <!-- Header -->
      <div class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0">
            <FbIcon name="users" size="18px" />
          </div>
          <div>
            <h3 class="text-sm font-bold text-gray-900 dark:text-white">Active Shared Links</h3>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">Manage public and temporary download links.</p>
          </div>
        </div>

        <button
          @click="isOpen = false"
          class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        >
          ✕
        </button>
      </div>

      <!-- Content -->
      <div class="p-6 overflow-y-auto flex-1 space-y-4">
        <!-- Empty State -->
        <div v-if="shares.length === 0 && !loading" class="py-16 text-center text-gray-400 dark:text-slate-500 space-y-2">
          <div class="w-12 h-12 rounded-2xl bg-gray-100 dark:bg-slate-900 mx-auto flex items-center justify-center text-gray-400">
            <FbIcon name="users" size="24px" />
          </div>
          <p class="font-bold text-gray-700 dark:text-slate-300">No active shared links</p>
          <p class="text-[11px]">Right click any file in the explorer to generate a share link.</p>
        </div>

        <!-- Shares List -->
        <div v-else class="space-y-2.5">
          <div
            v-for="share in shares"
            :key="share.id"
            class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 flex items-center justify-between gap-4"
          >
            <div class="truncate space-y-1">
              <div class="flex items-center space-x-2">
                <span class="font-bold text-gray-900 dark:text-white text-xs truncate">{{ share.path.split('/').pop() }}</span>
                <span v-if="share.has_password" class="px-2 py-0.5 rounded-md bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300 text-[10px] font-semibold">Password Protected</span>
                <span v-if="share.expires_at" class="px-2 py-0.5 rounded-md bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300 text-[10px] font-semibold">Expires {{ formatRelativeDate(share.expires_at) }}</span>
              </div>
              <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ share.path }}</p>
            </div>

            <!-- Actions -->
            <div class="flex items-center space-x-2 shrink-0">
              <button
                @click="copyShareLink(share.share_url)"
                class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded-xl text-xs transition shadow-xs cursor-pointer flex items-center space-x-1"
              >
                <span>Copy Link</span>
              </button>
              <button
                @click="revokeShare(share.id)"
                class="px-3 py-1.5 bg-red-50 hover:bg-red-100 dark:bg-red-950/40 dark:hover:bg-red-900/50 text-red-600 dark:text-red-400 font-semibold rounded-xl text-xs transition cursor-pointer"
              >
                Revoke
              </button>
            </div>
          </div>
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

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const uiStore = useUiStore();

const isOpen = ref(props.modelValue);
const shares = ref<any[]>([]);
const loading = ref(false);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      fetchShares();
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function fetchShares() {
  loading.value = true;
  try {
    const resp = await apiClient.get('/shares');
    shares.value = resp.data;
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to fetch shares', 'error');
  } finally {
    loading.value = false;
  }
}

async function copyShareLink(url: string) {
  const fullUrl = `${window.location.origin}${url}`;
  await navigator.clipboard.writeText(fullUrl);
  uiStore.showToast('Public link copied to clipboard!', 'success');
}

async function revokeShare(id: string) {
  try {
    await apiClient.delete(`/shares/${id}`);
    uiStore.showToast('Share link revoked', 'success');
    await fetchShares();
  } catch (err: any) {
    uiStore.showToast('Failed to revoke share', 'error');
  }
}

function formatRelativeDate(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}
</script>
