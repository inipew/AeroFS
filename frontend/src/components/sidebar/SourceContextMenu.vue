<template>
  <Teleport to="body">
    <div
      v-if="isOpen && connection"
      class="fixed inset-0 z-50 overflow-hidden"
      @click="closeMenu"
      @contextmenu.prevent="closeMenu"
    >
      <!-- Backdrop for mobile sheet & click-outside catcher -->
      <div
        class="fixed inset-0 transition-opacity"
        :class="uiStore.isMobile ? 'bg-black/60 backdrop-blur-xs' : 'bg-transparent'"
      ></div>

      <!-- Mobile Bottom Sheet Style -->
      <div
        v-if="uiStore.isMobile"
        class="fixed inset-x-0 bottom-0 z-50 bg-white dark:bg-slate-900 border-t border-gray-200/80 dark:border-slate-800 rounded-t-3xl shadow-2xl p-4 pb-8 space-y-3 animate-in slide-in-from-bottom duration-200 select-none font-sans"
        @click.stop
      >
        <!-- Pull Handle -->
        <div class="w-12 h-1 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto mb-2"></div>

        <!-- Connection Header Info -->
        <div class="flex items-center space-x-3 px-2 py-1">
          <div class="w-10 h-10 rounded-xl bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0">
            <FbIcon :name="connection.provider === 'local' ? 'folder' : 'share'" size="20px" />
          </div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center space-x-2">
              <h4 class="text-sm font-bold text-gray-900 dark:text-white truncate">
                {{ connection.name }}
              </h4>
              <span class="text-[10px] font-mono uppercase px-1.5 py-0.5 rounded bg-gray-100 dark:bg-slate-800 text-gray-500 dark:text-slate-400">
                {{ connection.provider }}
              </span>
            </div>
            <p class="text-xs text-gray-400 dark:text-slate-500 font-mono truncate">
              {{ connection.host ? `${connection.host}:${connection.port ?? ''}` : connection.base_path }}
            </p>
          </div>
        </div>

        <div class="divide-y divide-gray-100 dark:divide-slate-800/80 pt-1">
          <div class="py-1 space-y-1">
            <!-- Open in Active Panel -->
            <button
              @click="handleAction('open')"
              class="w-full flex items-center px-3 py-2.5 rounded-xl text-sm font-medium text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition active:scale-[0.98] cursor-pointer"
            >
              <FbIcon name="folder" size="18px" class="text-gray-400 mr-3" />
              <span>Open in Active Panel</span>
            </button>

            <!-- Open in Other Panel -->
            <button
              @click="handleAction('open-other')"
              class="w-full flex items-center px-3 py-2.5 rounded-xl text-sm font-medium text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition active:scale-[0.98] cursor-pointer"
            >
              <FbIcon name="panel-left" size="18px" class="text-gray-400 mr-3" />
              <span>Open in Other Panel</span>
            </button>

            <!-- Test Connection -->
            <button
              v-if="connection.provider !== 'local'"
              @click="handleAction('test')"
              :disabled="testing"
              class="w-full flex items-center px-3 py-2.5 rounded-xl text-sm font-medium text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition active:scale-[0.98] cursor-pointer disabled:opacity-50"
            >
              <FbIcon
                :name="testing ? 'refresh' : 'check-circle'"
                size="18px"
                class="text-gray-400 mr-3"
                :class="{ 'animate-spin text-blue-500': testing }"
              />
              <span>{{ testing ? 'Testing Connection...' : 'Test Connection' }}</span>
            </button>
          </div>

          <div class="py-1 space-y-1" v-if="canManage">
            <!-- Edit Connection -->
            <button
              v-if="connection.provider !== 'local'"
              @click="handleAction('edit')"
              class="w-full flex items-center px-3 py-2.5 rounded-xl text-sm font-medium text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition active:scale-[0.98] cursor-pointer"
            >
              <FbIcon name="rename" size="18px" class="text-gray-400 mr-3" />
              <span>Edit Connection</span>
            </button>

            <!-- Delete Connection -->
            <button
              v-if="connection.id !== 'local'"
              @click="handleAction('delete')"
              class="w-full flex items-center px-3 py-2.5 rounded-xl text-sm font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 transition active:scale-[0.98] cursor-pointer"
            >
              <FbIcon name="trash" size="18px" class="text-red-500 mr-3" />
              <span>Delete Connection</span>
            </button>
          </div>
        </div>

        <button
          @click="closeMenu"
          class="w-full mt-2 py-3 rounded-xl bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-300 font-semibold text-sm transition text-center cursor-pointer"
        >
          Cancel
        </button>
      </div>

      <!-- Desktop Floating Context Menu Style -->
      <div
        v-else
        ref="menuRef"
        :style="menuStyle"
        class="fixed z-50 w-56 bg-white/95 dark:bg-[#0f1422]/95 backdrop-blur-md border border-gray-200/80 dark:border-slate-800/90 rounded-2xl shadow-xl py-1.5 select-none font-sans text-xs animate-in fade-in zoom-in-95 duration-150"
        @click.stop
      >
        <!-- Header / Name -->
        <div class="px-3 py-1.5 mb-1 border-b border-gray-100 dark:border-slate-800/80 flex items-center justify-between">
          <span class="font-bold text-gray-900 dark:text-slate-100 truncate pr-2 max-w-[140px]">
            {{ connection.name }}
          </span>
          <span class="text-[9px] font-mono uppercase px-1.5 py-0.5 rounded bg-gray-100 dark:bg-slate-800 text-gray-500 dark:text-slate-400 shrink-0">
            {{ connection.provider }}
          </span>
        </div>

        <!-- Open in Active Panel -->
        <button
          @click="handleAction('open')"
          class="w-full flex items-center px-3 py-2 text-gray-700 dark:text-slate-200 hover:bg-blue-50 dark:hover:bg-blue-900/30 hover:text-blue-600 dark:hover:text-blue-400 transition cursor-pointer text-left group"
        >
          <FbIcon name="folder" size="15px" class="mr-2.5 text-gray-400 group-hover:text-blue-500 shrink-0" />
          <span class="font-medium truncate">Open in Active Panel</span>
        </button>

        <!-- Open in Other Panel -->
        <button
          @click="handleAction('open-other')"
          class="w-full flex items-center px-3 py-2 text-gray-700 dark:text-slate-200 hover:bg-blue-50 dark:hover:bg-blue-900/30 hover:text-blue-600 dark:hover:text-blue-400 transition cursor-pointer text-left group"
        >
          <FbIcon name="panel-left" size="15px" class="mr-2.5 text-gray-400 group-hover:text-blue-500 shrink-0" />
          <span class="font-medium truncate">Open in Other Panel</span>
        </button>

        <!-- Test Connection -->
        <button
          v-if="connection.provider !== 'local'"
          @click="handleAction('test')"
          :disabled="testing"
          class="w-full flex items-center px-3 py-2 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer text-left group disabled:opacity-50"
        >
          <FbIcon
            :name="testing ? 'refresh' : 'check-circle'"
            size="15px"
            class="mr-2.5 text-gray-400 group-hover:text-emerald-500 shrink-0"
            :class="{ 'animate-spin text-blue-500': testing }"
          />
          <span class="font-medium truncate">{{ testing ? 'Testing...' : 'Test Connection' }}</span>
        </button>

        <!-- Divider for Management Actions -->
        <div v-if="canManage && (connection.provider !== 'local' || connection.id !== 'local')" class="my-1 border-t border-gray-100 dark:border-slate-800/80"></div>

        <!-- Edit Connection -->
        <button
          v-if="canManage && connection.provider !== 'local'"
          @click="handleAction('edit')"
          class="w-full flex items-center px-3 py-2 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer text-left group"
        >
          <FbIcon name="rename" size="15px" class="mr-2.5 text-gray-400 group-hover:text-gray-700 dark:group-hover:text-slate-200 shrink-0" />
          <span class="font-medium truncate">Edit Connection</span>
        </button>

        <!-- Delete Connection -->
        <button
          v-if="canManage && connection.id !== 'local'"
          @click="handleAction('delete')"
          class="w-full flex items-center px-3 py-2 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 transition cursor-pointer text-left group"
        >
          <FbIcon name="trash" size="15px" class="mr-2.5 text-red-500 group-hover:text-red-600 shrink-0" />
          <span class="font-medium truncate">Delete Connection</span>
        </button>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useUiStore } from '../../stores/uiStore';
import { useAuthStore } from '../../stores/authStore';
import type { Connection } from '../../types/connection';

const props = defineProps<{
  modelValue: boolean;
  x: number;
  y: number;
  connection: Connection | null;
  testing?: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'open', conn: Connection): void;
  (e: 'openOther', conn: Connection): void;
  (e: 'test', conn: Connection): void;
  (e: 'edit', conn: Connection): void;
  (e: 'delete', conn: Connection): void;
}>();

const uiStore = useUiStore();
const authStore = useAuthStore();

const isOpen = computed({
  get: () => props.modelValue,
  set: (val: boolean) => emit('update:modelValue', val),
});

const canManage = computed(() => {
  return authStore.user?.is_admin ?? false;
});

const menuRef = ref<HTMLElement | null>(null);
const computedPos = ref({ x: props.x, y: props.y });

watch(
  () => [props.modelValue, props.x, props.y],
  async ([open]) => {
    if (open) {
      await nextTick();
      adjustPosition(props.x, props.y);
    }
  }
);

function adjustPosition(initialX: number, initialY: number) {
  const pad = 10;
  const menuWidth = menuRef.value?.offsetWidth || 224; // 14rem (w-56)
  const menuHeight = menuRef.value?.offsetHeight || 220;

  let x = initialX;
  let y = initialY;

  if (x + menuWidth > window.innerWidth - pad) {
    x = window.innerWidth - pad - menuWidth;
  }
  if (x < pad) {
    x = pad;
  }

  if (y + menuHeight > window.innerHeight - pad) {
    y = window.innerHeight - pad - menuHeight;
  }
  if (y < pad) {
    y = pad;
  }

  computedPos.value = { x, y };
}

const menuStyle = computed(() => ({
  left: `${computedPos.value.x}px`,
  top: `${computedPos.value.y}px`,
}));

function closeMenu() {
  isOpen.value = false;
}

function handleAction(action: 'open' | 'open-other' | 'test' | 'edit' | 'delete') {
  if (!props.connection) return;
  const conn = props.connection;
  
  if (action === 'test') {
    emit('test', conn);
    return;
  }

  closeMenu();

  if (action === 'open') {
    emit('open', conn);
  } else if (action === 'open-other') {
    emit('openOther', conn);
  } else if (action === 'edit') {
    emit('edit', conn);
  } else if (action === 'delete') {
    emit('delete', conn);
  }
}
</script>
