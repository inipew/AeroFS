<template>
  <!-- Toast Notifications Container with Apple Spring Transitions -->
  <div class="fixed bottom-5 right-5 z-[9999] flex flex-col space-y-2 pointer-events-none max-w-sm w-full select-none font-sans">
    <TransitionGroup
      enter-active-class="transition duration-[280ms] [transition-timing-function:var(--ease-spring)] transform"
      enter-from-class="translate-y-3 scale-95 opacity-0 sm:translate-y-0 sm:translate-x-3"
      enter-to-class="translate-y-0 scale-100 opacity-100 sm:translate-x-0"
      leave-active-class="transition duration-[180ms] [transition-timing-function:var(--ease-apple-in)] transform"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-95"
    >
      <div
        v-for="toast in uiStore.toasts"
        :key="toast.id"
        :class="[
          'pointer-events-auto flex items-center space-x-3 px-4 py-3 rounded-2xl shadow-2xl backdrop-blur-xl border text-xs font-semibold ring-1 ring-black/5 dark:ring-white/10 transition-[opacity,transform,background-color,border-color] duration-standard ease-spring',
          toast.type === 'success' ? 'bg-emerald-50/95 dark:bg-emerald-950/90 border-emerald-200 dark:border-emerald-800/80 text-emerald-900 dark:text-emerald-200' : '',
          toast.type === 'error' ? 'bg-rose-50/95 dark:bg-rose-950/90 border-rose-200 dark:border-rose-800/80 text-rose-900 dark:text-rose-200' : '',
          toast.type === 'warning' ? 'bg-amber-50/95 dark:bg-amber-950/90 border-amber-200 dark:border-amber-800/80 text-amber-900 dark:text-amber-200' : '',
          toast.type === 'info' ? 'bg-blue-50/95 dark:bg-slate-900/90 border-blue-200 dark:border-slate-800/80 text-gray-900 dark:text-slate-100' : ''
        ]"
      >
        <div
          class="w-6 h-6 rounded-full flex items-center justify-center shrink-0"
          :class="[
            toast.type === 'success' ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400' : '',
            toast.type === 'error' ? 'bg-rose-500/20 text-rose-600 dark:text-rose-400' : '',
            toast.type === 'warning' ? 'bg-amber-500/20 text-amber-600 dark:text-amber-400' : '',
            toast.type === 'info' ? 'bg-blue-500/20 text-blue-600 dark:text-blue-400' : ''
          ]"
        >
          <FbIcon v-if="toast.type === 'success'" name="check" size="13px" />
          <FbIcon v-else-if="toast.type === 'error'" name="x" size="13px" />
          <FbIcon v-else-if="toast.type === 'warning'" name="info" size="13px" />
          <FbIcon v-else name="info" size="13px" />
        </div>
        <span class="flex-1 leading-snug">{{ toast.message }}</span>
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import FbIcon from '../components/common/FbIcon.vue';
import { useUiStore } from '../stores/uiStore';

const uiStore = useUiStore();
</script>
