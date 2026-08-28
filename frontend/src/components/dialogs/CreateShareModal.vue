<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs"
      @click="isOpen = false"
    >
      <div
        class="modal-card bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl space-y-4"
        @click.stop
      >
      <div class="flex items-center space-x-3">
        <div class="w-10 h-10 rounded-2xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon name="users" size="20px" />
        </div>
        <div>
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">Create Share Link</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs truncate max-w-[250px]">{{ path }}</p>
        </div>
      </div>

      <form @submit.prevent="handleCreateShare" class="space-y-3.5">
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Link Expiration</label>
          <select
            v-model="expiresInHours"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white text-xs cursor-pointer shadow-inner"
          >
            <option :value="1">1 Hour</option>
            <option :value="24">1 Day (24 Hours)</option>
            <option :value="168">7 Days</option>
            <option :value="720">30 Days</option>
            <option :value="null">Never Expire (Permanent)</option>
          </select>
        </div>

        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Password Protection (Optional)</label>
          <input
            v-model="password"
            type="password"
            placeholder="Leave empty for public link"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
          />
        </div>

        <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
          <button
            type="button"
            @click="isOpen = false"
            class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="submit"
            :disabled="creating"
            class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 cursor-pointer flex items-center space-x-1.5"
          >
            <span v-if="creating" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>{{ creating ? 'Creating...' : 'Generate Share Link' }}</span>
          </button>
        </div>
      </form>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useUiStore } from '../../stores/uiStore';

const props = defineProps<{
  modelValue: boolean;
  connectionId: string;
  path: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const uiStore = useUiStore();

const isOpen = ref(props.modelValue);
const expiresInHours = ref<number | null>(24);
const password = ref('');
const creating = ref(false);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      password.value = '';
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function handleCreateShare() {
  creating.value = true;
  try {
    const resp = await apiClient.post('/shares', {
      connection_id: props.connectionId,
      path: props.path,
      password: password.value.trim() ? password.value : null,
      expires_in_hours: expiresInHours.value,
    });

    const shareUrl = `${window.location.origin}${resp.data.share_url}`;
    await navigator.clipboard.writeText(shareUrl);
    uiStore.showToast('Share link created and copied to clipboard!', 'success');
    isOpen.value = false;
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to create share', 'error');
  } finally {
    creating.value = false;
  }
}
</script>
