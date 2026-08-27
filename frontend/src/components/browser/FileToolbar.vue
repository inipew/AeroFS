<template>
  <div class="p-3 bg-slate-900 border-b border-slate-800 flex flex-wrap items-center justify-between gap-3 text-xs select-none">
    <!-- Left: Navigation & Path -->
    <div class="flex items-center space-x-2 flex-1 min-w-[300px]">
      <!-- Back / Forward / Up / Refresh -->
      <div class="flex items-center space-x-1 bg-slate-950/60 p-0.5 rounded border border-slate-800">
        <button
          @click="fileStore.goBack()"
          :disabled="fileStore.historyIndex <= 0"
          class="p-1.5 rounded hover:bg-slate-800 text-slate-300 disabled:opacity-30 disabled:hover:bg-transparent transition"
          title="Back"
        >
          &larr;
        </button>
        <button
          @click="fileStore.goForward()"
          :disabled="fileStore.historyIndex >= fileStore.history.length - 1"
          class="p-1.5 rounded hover:bg-slate-800 text-slate-300 disabled:opacity-30 disabled:hover:bg-transparent transition"
          title="Forward"
        >
          &rarr;
        </button>
        <button
          @click="fileStore.navigateUp()"
          :disabled="fileStore.currentPath === '/' || fileStore.currentPath === ''"
          class="p-1.5 rounded hover:bg-slate-800 text-slate-300 disabled:opacity-30 disabled:hover:bg-transparent transition"
          title="Up"
        >
          &uarr;
        </button>
        <button
          @click="fileStore.fetchEntries()"
          class="p-1.5 rounded hover:bg-slate-800 text-slate-300 transition"
          title="Refresh"
        >
          &#x21bb;
        </button>
      </div>

      <!-- Breadcrumb Component -->
      <div class="flex-1 min-w-[180px]">
        <Breadcrumb />
      </div>
    </div>

    <!-- Right: Actions & View controls -->
    <div class="flex items-center space-x-2">
      <!-- Search Input -->
      <div class="relative">
        <input
          v-model="fileStore.searchQuery"
          type="text"
          placeholder="Filter files..."
          class="bg-slate-950 border border-slate-800 rounded px-2.5 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 w-36 sm:w-48 transition"
        />
        <button
          v-if="fileStore.searchQuery"
          @click="fileStore.searchQuery = ''"
          class="absolute right-2 top-1.5 text-slate-500 hover:text-white"
        >
          &times;
        </button>
      </div>

      <!-- Action Buttons -->
      <button
        @click="uiStore.openCreate('file')"
        class="flex items-center space-x-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded border border-slate-700 transition"
      >
        <span>+</span>
        <span>File</span>
      </button>

      <button
        @click="uiStore.openCreate('directory')"
        class="flex items-center space-x-1.5 px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded border border-slate-700 transition"
      >
        <span>+</span>
        <span>Folder</span>
      </button>

      <button
        @click="uiStore.openUpload()"
        class="flex items-center space-x-1.5 px-2.5 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded font-medium shadow-sm transition"
      >
        <span>Upload</span>
      </button>

      <button
        v-if="fileStore.selectedCount > 0"
        @click="uiStore.openDelete(fileStore.selectedEntries)"
        class="flex items-center space-x-1 px-2.5 py-1.5 bg-red-600/20 hover:bg-red-600/30 text-red-400 border border-red-500/30 rounded transition"
      >
        <span>Delete ({{ fileStore.selectedCount }})</span>
      </button>

      <!-- View Mode Toggle -->
      <div class="flex items-center bg-slate-950 border border-slate-800 rounded p-0.5 ml-2">
        <button
          @click="fileStore.viewMode = 'list'"
          :class="[
            'px-2 py-1 rounded text-xs transition',
            fileStore.viewMode === 'list' ? 'bg-slate-800 text-indigo-300 font-medium' : 'text-slate-500 hover:text-slate-300'
          ]"
          title="List View"
        >
          List
        </button>
        <button
          @click="fileStore.viewMode = 'grid'"
          :class="[
            'px-2 py-1 rounded text-xs transition',
            fileStore.viewMode === 'grid' ? 'bg-slate-800 text-indigo-300 font-medium' : 'text-slate-500 hover:text-slate-300'
          ]"
          title="Grid View"
        >
          Grid
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import Breadcrumb from './Breadcrumb.vue';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';

const fileStore = useFileStore();
const uiStore = useUiStore();
</script>
