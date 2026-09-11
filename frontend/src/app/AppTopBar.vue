<template>
  <header class="h-14 md:h-16 bg-white/95 dark:bg-[#0b0f19]/95 backdrop-blur-md border-b border-gray-200/80 dark:border-slate-800/80 px-3 sm:px-4 md:px-5 flex items-center justify-between text-gray-800 dark:text-slate-100 select-none sticky top-0 z-30 transition-colors duration-150">
    <!-- LEFT: Mobile Menu Toggle & AeroFS Brand -->
    <div class="flex items-center space-x-2.5 sm:space-x-3">
      <!-- Mobile Sidebar Toggle -->
      <button
        v-if="uiStore.isMobile"
        @click="uiStore.isMobileSidebarOpen = true"
        class="p-2 -ml-1 text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer shrink-0"
        title="Open Navigation Menu"
      >
        <FbIcon name="menu" size="19px" />
      </button>

      <!-- Brand Logo & Workspace Title -->
      <div class="flex items-center space-x-2.5 shrink-0 font-bold text-gray-900 dark:text-white text-sm">
        <div class="w-8 h-8 rounded-xl bg-gradient-to-tr from-blue-600 to-indigo-500 flex items-center justify-center text-white shadow-xs">
          <FbIcon name="folder" size="16px" />
        </div>
        <div class="flex flex-col">
          <span class="tracking-tight leading-tight">AeroFS</span>
          <span class="text-[10px] text-gray-400 dark:text-slate-500 font-normal leading-tight hidden sm:inline">Workspace</span>
        </div>
      </div>
    </div>

    <!-- CENTER: Spotlight Search / Command Palette Pill (Desktop) -->
    <div v-if="!uiStore.isMobile" class="hidden md:flex items-center justify-center flex-1 px-4 max-w-xs lg:max-w-md">
      <button
        @click="uiStore.toggleCommandPalette()"
        class="apple-search-pill bg-gray-100/80 dark:bg-slate-800/60 hover:bg-gray-100 dark:hover:bg-slate-800 border border-gray-200/80 dark:border-slate-700/60 hover:border-blue-500/40 dark:hover:border-blue-400/40 px-3.5 py-1.5 rounded-2xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 flex items-center justify-between text-xs transition shadow-2xs w-48 sm:w-56 hover:w-64 cursor-pointer group select-none"
        title="Command Palette & Instant Search (⌘K / Ctrl+K)"
      >
        <div class="flex items-center space-x-2 truncate">
          <FbIcon name="search" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
          <span class="text-gray-400 font-medium truncate">Search or jump to...</span>
        </div>
        <kbd class="px-1.5 py-0.5 rounded-md bg-white dark:bg-slate-900 text-[10px] text-gray-500 dark:text-slate-400 border border-gray-200 dark:border-slate-700 font-mono shadow-2xs">⌘K</kbd>
      </button>
    </div>

    <!-- RIGHT: Global Chrome Action Controls -->
    <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
      <!-- Mobile Search Dialog Trigger -->
      <button
        v-if="uiStore.isMobile"
        @click="overlayStore.open({ type: 'search' })"
        class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        title="Search files"
      >
        <FbIcon name="search" size="18px" />
      </button>

      <!-- Desktop Dual-Pane Layout Toggle -->
      <button
        v-if="!uiStore.isMobile"
        @click="workspaceStore.setDualPane(!workspaceStore.isDualPane)"
        :class="[
          'p-1.5 rounded-xl border transition cursor-pointer active:scale-95 duration-fast ease-spring',
          workspaceStore.isDualPane
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
            : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        :title="workspaceStore.isDualPane ? 'Disable Dual Pane (Alt+D)' : 'Enable Dual Pane (Alt+D)'"
      >
        <FbIcon name="panel-right" size="15px" />
      </button>

      <!-- Transfer Drawer Badge & Trigger -->
      <button
        @click="transferStore.isDrawerOpen = !transferStore.isDrawerOpen"
        :class="[
          'p-1.5 sm:px-2.5 sm:py-1.5 rounded-xl border transition cursor-pointer active:scale-95 duration-fast ease-spring flex items-center space-x-1.5 text-xs font-medium relative',
          transferStore.activeCount > 0
            ? 'border-blue-500/40 bg-blue-50/80 dark:bg-blue-950/40 text-blue-600 dark:text-blue-400'
            : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        title="Transfer Manager"
      >
        <FbIcon name="refresh" size="14px" :class="{ 'animate-spin': transferStore.activeCount > 0 }" />
        <span class="hidden sm:inline">Transfers</span>
        <span
          v-if="transferStore.activeCount > 0"
          class="px-1.5 py-0.2 rounded-full bg-blue-600 text-white text-[10px] font-bold"
        >
          {{ transferStore.activeCount }}
        </span>
      </button>

      <!-- Settings Dialog Trigger -->
      <button
        @click="overlayStore.open({ type: 'settings' })"
        class="p-1.5 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring"
        title="Settings"
      >
        <FbIcon name="settings" size="15px" />
      </button>

      <!-- User Menu / Logout -->
      <div v-if="authStore.user" class="relative">
        <button
          @click="isUserMenuOpen = !isUserMenuOpen"
          class="flex items-center space-x-1.5 p-1 sm:px-2 sm:py-1 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          :title="`Logged in as ${authStore.user.username}`"
        >
          <div class="w-6 h-6 rounded-full bg-blue-100 dark:bg-blue-900/60 text-blue-600 dark:text-blue-300 font-bold text-xs flex items-center justify-center">
            {{ authStore.user.username.charAt(0).toUpperCase() }}
          </div>
          <span class="text-xs font-medium text-gray-700 dark:text-slate-200 hidden md:inline truncate max-w-[100px]">
            {{ authStore.user.username }}
          </span>
        </button>

        <Transition name="ios-popover">
          <div
            v-if="isUserMenuOpen"
            @click="isUserMenuOpen = false"
            class="absolute right-0 mt-2 w-44 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-1"
          >
            <div class="px-2.5 py-1 border-b border-gray-100 dark:border-slate-800/80">
              <p class="font-semibold truncate">{{ authStore.user.username }}</p>
              <p v-if="authStore.isAdmin" class="text-[10px] text-blue-600 dark:text-blue-400 font-bold uppercase tracking-wider">Admin</p>
            </div>
            <button
              @click="authStore.logout()"
              class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-xl hover:bg-rose-50 dark:hover:bg-rose-950/40 text-rose-600 dark:text-rose-400 transition text-left cursor-pointer"
            >
              <FbIcon name="x" size="13px" />
              <span>Sign Out</span>
            </button>
          </div>
        </Transition>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import FbIcon from '../components/common/FbIcon.vue';
import { useUiStore } from '../stores/uiStore';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { useTransferStore } from '../stores/transferStore';
import { useOverlayStore } from '../overlays/overlayStore';
import { useAuthStore } from '../stores/authStore';

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();
const transferStore = useTransferStore();
const overlayStore = useOverlayStore();
const authStore = useAuthStore();

const isUserMenuOpen = ref(false);
</script>
