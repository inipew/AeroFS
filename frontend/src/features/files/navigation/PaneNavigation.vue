<template>
  <div class="flex items-center space-x-2 truncate flex-1 min-w-0">
    <!-- Segmented Navigation Pill: Back & Forward (‹  ›) -->
    <div class="ios-segmented-group shrink-0">
      <button
        type="button"
        @click="$emit('back')"
        :disabled="!canGoBack"
        class="ios-segmented-item p-1.5"
        title="Back (Alt+Left)"
      >
        <FbIcon name="chevron-left" size="13px" />
      </button>

      <button
        type="button"
        @click="$emit('forward')"
        :disabled="!canGoForward"
        class="ios-segmented-item p-1.5"
        title="Forward (Alt+Right)"
      >
        <FbIcon name="chevron-right" size="13px" />
      </button>
    </div>

    <!-- Direct Path Editing vs Breadcrumbs -->
    <div v-if="isEditing" class="flex-1 min-w-0">
      <LocationInput
        :initial-path="path"
        :suggestions="suggestions"
        @submit="handlePathSubmit"
        @cancel="isEditing = false"
      />
    </div>

    <div v-else class="flex items-center space-x-1 flex-1 min-w-0">
      <Breadcrumbs
        :path="path"
        :connection-name="connectionName"
        :connection-provider="connectionProvider"
        :connection-status="connectionStatus"
        @navigate="handleNavigate"
        @edit-path="isEditing = true"
      />

      <!-- Quick Edit Path Trigger Button -->
      <button
        type="button"
        @click="isEditing = true"
        class="p-1 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-200/50 dark:hover:bg-slate-700/50 transition cursor-pointer shrink-0 opacity-0 group-hover:opacity-100 focus:opacity-100"
        title="Edit path directly (Ctrl+L)"
      >
        <FbIcon name="rename" size="12px" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import FbIcon from '../../../components/common/FbIcon.vue';
import Breadcrumbs from './Breadcrumbs.vue';
import LocationInput from './LocationInput.vue';

withDefaults(
  defineProps<{
    canGoBack: boolean;
    canGoForward: boolean;
    path: string;
    connectionName?: string;
    connectionProvider?: string;
    connectionStatus?: string;
    suggestions?: string[];
  }>(),
  {
    connectionName: 'Local Storage',
    connectionProvider: 'local',
    connectionStatus: 'connected',
    suggestions: () => [],
  }
);

const emit = defineEmits<{
  (e: 'back'): void;
  (e: 'forward'): void;
  (e: 'navigate', path: string): void;
  (e: 'refresh'): void;
}>();

const isEditing = ref(false);

function handleNavigate(newPath: string) {
  isEditing.value = false;
  emit('navigate', newPath);
}

function handlePathSubmit(newPath: string) {
  isEditing.value = false;
  emit('navigate', newPath);
}

defineExpose({
  openAddressBar: () => {
    isEditing.value = true;
  },
});
</script>
