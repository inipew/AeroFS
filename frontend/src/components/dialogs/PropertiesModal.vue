<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      :class="[
        'fixed inset-0 z-50 bg-black/60 backdrop-blur-sm select-none font-sans text-xs',
        uiStore.isMobile ? 'flex flex-col justify-end p-0' : 'flex items-center justify-center p-3 sm:p-6'
      ]"
      @click="closeModal"
    >
      <div
        :class="[
          'modal-card bg-white/95 dark:bg-[#0f172a]/95 backdrop-blur-2xl border border-gray-200/90 dark:border-slate-700/80 flex flex-col shadow-2xl overflow-hidden ring-1 ring-black/5 dark:ring-white/10',
          uiStore.isMobile
            ? 'w-full rounded-t-3xl rounded-b-none border-b-0 max-h-[90vh] pb-safe'
            : 'max-w-xl w-full rounded-3xl max-h-[85vh]'
        ]"
        @click.stop
      >
        <div v-if="uiStore.isMobile" class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto mt-3 mb-1"></div>
        <div class="p-5 sm:p-6 bg-gradient-to-b from-gray-50/80 to-transparent dark:from-slate-900/50 dark:to-transparent border-b border-gray-100 dark:border-slate-800/80 flex items-start justify-between gap-4 shrink-0">
          <div class="flex items-center space-x-3.5 min-w-0 flex-1">
            <div :class="['w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border bg-gradient-to-br shadow-inner transition-transform duration-fast ease-spring', itemVisual.gradient]">
              <FbIcon :name="itemVisual.icon" size="24px" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2 mb-0.5">
                <h3 class="text-base font-bold text-gray-900 dark:text-white truncate" :title="displayName">{{ displayName }}</h3>
                <span :class="['text-[10px] font-bold px-2 py-0.5 rounded-full border shrink-0', itemVisual.badgeColor]">{{ itemVisual.badge }}</span>
              </div>
              <div class="flex items-center space-x-1 text-[11px] text-gray-400 dark:text-slate-400 font-mono">
                <span class="truncate" :title="meta?.path || path">{{ meta?.path || path }}</span>
              </div>
            </div>
          </div>
          <button type="button" @click="closeModal" class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer shrink-0" title="Close (Esc)">
            <FbIcon name="x" size="16px" />
          </button>
        </div>

        <div class="px-5 sm:px-6 pt-3 pb-1 bg-white/50 dark:bg-[#0f172a]/50 shrink-0">
          <div class="p-1 bg-gray-100/90 dark:bg-slate-900/90 rounded-2xl flex gap-1 border border-gray-200/60 dark:border-slate-800">
            <button type="button" @click="activeTab = 'general'" :class="['flex-1 py-2 rounded-xl transition duration-fast ease-spring flex items-center justify-center space-x-2 text-xs font-semibold cursor-pointer select-none', activeTab === 'general' ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs border border-gray-200/50 dark:border-slate-700/60' : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200 hover:bg-white/40 dark:hover:bg-slate-800/40']">
              <FbIcon name="info" size="14px" /><span>General Info</span>
            </button>
            <button type="button" @click="activeTab = 'permissions'" :class="['flex-1 py-2 rounded-xl transition duration-fast ease-spring flex items-center justify-center space-x-2 text-xs font-semibold cursor-pointer select-none', activeTab === 'permissions' ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs border border-gray-200/50 dark:border-slate-700/60' : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200 hover:bg-white/40 dark:hover:bg-slate-800/40']">
              <FbIcon name="shield" size="14px" /><span>Permissions (CHMOD)</span>
            </button>
          </div>
        </div>

        <div class="p-5 sm:p-6 overflow-y-auto flex-1 space-y-4 bg-white/40 dark:bg-[#0f172a]/40">
          <div v-if="loading" class="py-14 flex flex-col items-center justify-center space-y-2.5 text-gray-400">
            <div class="animate-spin rounded-full h-7 w-7 border-2 border-blue-600 border-t-transparent"></div>
            <span class="font-medium text-xs">Loading item properties...</span>
          </div>
          <div v-else-if="errorMsg" class="py-12 flex flex-col items-center justify-center space-y-3 text-rose-500">
            <div class="w-12 h-12 rounded-2xl bg-rose-50 dark:bg-rose-950/50 text-rose-600 dark:text-rose-400 flex items-center justify-center border border-rose-200/60 dark:border-rose-900/60"><FbIcon name="frown" size="24px" /></div>
            <p class="text-xs font-medium">{{ errorMsg }}</p>
            <button type="button" @click="fetchMetadata" class="px-3.5 py-1.5 bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-200 rounded-xl text-xs hover:bg-gray-200 dark:hover:bg-slate-700 transition cursor-pointer font-semibold shadow-2xs">Try Again</button>
          </div>

          <template v-else-if="meta">
            <div v-if="activeTab === 'general'" class="space-y-4 font-sans text-xs">
              <div class="grid grid-cols-3 gap-2.5">
                <div class="p-3.5 bg-gray-50/90 dark:bg-slate-900/60 rounded-2xl border border-gray-200/80 dark:border-slate-800 flex flex-col justify-between">
                  <span class="text-[10px] font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500">Size</span>
                  <div class="my-1"><p class="text-sm sm:text-base font-bold text-gray-900 dark:text-white truncate">{{ formatBytes(meta.size) }}</p><p class="text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ (meta.size ?? 0).toLocaleString() }} B</p></div>
                </div>
                <div class="p-3.5 bg-gray-50/90 dark:bg-slate-900/60 rounded-2xl border border-gray-200/80 dark:border-slate-800 flex flex-col justify-between">
                  <span class="text-[10px] font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500">Type</span>
                  <div class="my-1"><p class="text-sm sm:text-base font-bold text-gray-900 dark:text-white capitalize truncate">{{ meta.kind }}</p><p class="text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate" :title="meta.mime_type || ''">{{ meta.mime_type ? meta.mime_type.split('/')[1] || meta.mime_type : (meta.kind === 'directory' ? 'folder' : 'binary') }}</p></div>
                </div>
                <div class="p-3.5 bg-gray-50/90 dark:bg-slate-900/60 rounded-2xl border border-gray-200/80 dark:border-slate-800 flex flex-col justify-between">
                  <span class="text-[10px] font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500">Access</span>
                  <div class="my-1"><p class="text-sm sm:text-base font-bold text-blue-600 dark:text-blue-400 font-mono truncate">{{ octalMode }}</p><p class="text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ permissionString }}</p></div>
                </div>
              </div>

              <div class="p-4 bg-gray-50/90 dark:bg-slate-900/60 rounded-2xl border border-gray-200/80 dark:border-slate-800 space-y-3">
                <div>
                  <span class="block text-[11px] font-semibold text-gray-500 dark:text-slate-400 mb-1.5">Location Path</span>
                  <div class="flex items-center justify-between bg-white dark:bg-slate-950 border border-gray-200/90 dark:border-slate-800 rounded-xl px-3 py-2 shadow-inner">
                    <span class="font-mono text-gray-800 dark:text-slate-200 text-xs truncate max-w-[340px]" :title="meta.path">{{ meta.path }}</span>
                    <button type="button" @click="copyPath(meta.path)" class="ml-2 px-2.5 py-1 rounded-lg text-xs font-semibold transition cursor-pointer flex items-center space-x-1 shrink-0 active:scale-95" :class="isPathCopied ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-950/50 dark:text-emerald-400' : 'text-gray-600 hover:text-blue-600 hover:bg-gray-100 dark:hover:bg-slate-800 dark:text-slate-300'" :title="isPathCopied ? 'Copied to clipboard!' : 'Copy path'">
                      <FbIcon :name="isPathCopied ? 'check' : 'copy'" size="12px" /><span>{{ isPathCopied ? 'Copied' : 'Copy' }}</span>
                    </button>
                  </div>
                </div>
                <div v-if="meta.symlink_target" class="py-1.5 border-t border-gray-100 dark:border-slate-800/80 flex items-center justify-between"><span class="text-gray-500 dark:text-slate-400">Points to (Symlink):</span><span class="font-mono text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/40 px-2 py-0.5 rounded-lg border border-amber-200/60 dark:border-amber-800/60 truncate max-w-[240px]">→ {{ meta.symlink_target }}</span></div>
                <div class="py-1.5 border-t border-gray-100 dark:border-slate-800/80 flex items-center justify-between"><span class="text-gray-500 dark:text-slate-400">Content MIME:</span><span class="font-mono text-gray-800 dark:text-slate-200 bg-gray-100 dark:bg-slate-800 px-2 py-0.5 rounded-lg text-[11px]">{{ meta.mime_type || (meta.kind === 'directory' ? 'inode/directory' : 'application/octet-stream') }}</span></div>
                <div class="py-1.5 border-t border-gray-100 dark:border-slate-800/80 flex items-center justify-between"><span class="text-gray-500 dark:text-slate-400 flex items-center gap-1.5"><FbIcon name="clock" size="13px" class="text-gray-400" /><span>Last Modified:</span></span><span class="font-mono text-gray-800 dark:text-slate-200">{{ formatDate(meta.modified_at) }}</span></div>
                <div v-if="meta.created_at" class="py-1.5 border-t border-gray-100 dark:border-slate-800/80 flex items-center justify-between"><span class="text-gray-500 dark:text-slate-400 flex items-center gap-1.5"><FbIcon name="clock" size="13px" class="text-gray-400" /><span>Created:</span></span><span class="font-mono text-gray-800 dark:text-slate-200">{{ formatDate(meta.created_at) }}</span></div>
                <div v-if="meta.etag" class="py-1.5 border-t border-gray-100 dark:border-slate-800/80 flex items-center justify-between"><span class="text-gray-500 dark:text-slate-400">ETag / Concurrency:</span><div class="flex items-center space-x-1.5 truncate max-w-[260px]"><span class="font-mono text-gray-500 dark:text-slate-400 text-[11px] truncate" :title="meta.etag">{{ meta.etag }}</span><button type="button" @click="copyEtag(meta.etag)" class="p-1 text-gray-400 hover:text-blue-600 rounded transition cursor-pointer" :title="isEtagCopied ? 'Copied!' : 'Copy ETag'"><FbIcon :name="isEtagCopied ? 'check' : 'copy'" size="11px" /></button></div></div>
              </div>
            </div>

            <div v-if="activeTab === 'permissions'" class="space-y-4 font-sans text-xs">
              <div class="p-4 bg-gray-50/90 dark:bg-slate-900/60 rounded-2xl border border-gray-200/80 dark:border-slate-800 space-y-4">
                <div class="flex items-center justify-between">
                  <div><h4 class="font-bold text-gray-900 dark:text-white flex items-center gap-1.5"><FbIcon name="shield" size="14px" class="text-blue-500" /><span>Unix Permissions</span></h4><p class="text-[11px] text-gray-400 dark:text-slate-500">Read (r), Write (w), and Execute (x) mode.</p></div>
                  <div class="flex items-center space-x-2"><span class="font-mono text-xs px-2.5 py-1 rounded-xl bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300 font-semibold select-none">{{ permissionString }}</span><div class="flex items-center space-x-1"><input v-model="octalMode" @input="onOctalInput" type="text" maxlength="4" class="w-16 bg-white dark:bg-slate-950 border border-blue-300 dark:border-blue-700 rounded-xl px-2 py-1 text-center font-mono font-bold text-blue-600 dark:text-blue-400 focus:outline-none focus:ring-2 focus:ring-blue-500/30 text-sm shadow-inner" /></div></div>
                </div>

                <div>
                  <span class="block text-[11px] font-semibold text-gray-500 dark:text-slate-400 mb-1.5">Quick Presets</span>
                  <div class="grid grid-cols-2 sm:grid-cols-4 gap-1.5">
                    <button v-for="preset in ['0644','0755','0444','0700']" :key="preset" type="button" @click="applyPreset(preset)" :class="['px-2.5 py-1.5 rounded-xl border text-[11px] font-mono font-medium transition cursor-pointer text-center', octalMode === preset ? 'bg-blue-50 dark:bg-blue-950/50 border-blue-500 text-blue-600 dark:text-blue-300 font-bold shadow-2xs' : 'bg-white dark:bg-slate-950 border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700 text-gray-700 dark:text-slate-300']">{{ preset }}</button>
                  </div>
                </div>

                <div class="border border-gray-200/90 dark:border-slate-800 rounded-2xl overflow-hidden bg-white dark:bg-slate-950 shadow-2xs">
                  <table class="w-full text-center border-collapse text-xs">
                    <thead class="bg-gray-50/90 dark:bg-slate-900/90 text-gray-500 dark:text-slate-400 text-[11px] font-semibold border-b border-gray-200 dark:border-slate-800"><tr><th class="py-2.5 px-4 text-left font-bold">Scope</th><th class="py-2.5 px-3">Read (4)</th><th class="py-2.5 px-3">Write (2)</th><th class="py-2.5 px-3">Execute (1)</th></tr></thead>
                    <tbody class="divide-y divide-gray-100 dark:divide-slate-800/80">
                      <tr v-for="scope in permissionScopes" :key="scope.key" class="hover:bg-blue-50/30 dark:hover:bg-blue-950/20 transition">
                        <td class="py-3 px-4 text-left font-bold text-gray-900 dark:text-white"><span>{{ scope.label }}</span></td>
                        <td class="py-3 px-3"><input type="checkbox" v-model="permState[scope.key].r" @change="recalcOctal" class="w-4 h-4 rounded text-blue-600 focus:ring-blue-500 cursor-pointer accent-blue-600" /></td>
                        <td class="py-3 px-3"><input type="checkbox" v-model="permState[scope.key].w" @change="recalcOctal" class="w-4 h-4 rounded text-blue-600 focus:ring-blue-500 cursor-pointer accent-blue-600" /></td>
                        <td class="py-3 px-3"><input type="checkbox" v-model="permState[scope.key].x" @change="recalcOctal" class="w-4 h-4 rounded text-blue-600 focus:ring-blue-500 cursor-pointer accent-blue-600" /></td>
                      </tr>
                    </tbody>
                  </table>
                </div>

                <div v-if="meta.kind === 'directory' && canApplyRecursive" class="flex items-center space-x-2.5 p-3 rounded-xl bg-amber-50/60 dark:bg-amber-950/30 border border-amber-200/60 dark:border-amber-800/50">
                  <input type="checkbox" id="recCheck" v-model="applyRecursive" class="w-4 h-4 rounded text-blue-600 cursor-pointer accent-blue-600" />
                  <label for="recCheck" class="text-gray-700 dark:text-slate-300 font-medium cursor-pointer text-xs select-none">Apply permissions recursively to all enclosed files and subfolders</label>
                </div>
                <div v-else-if="meta.kind === 'directory'" class="p-3 rounded-xl bg-slate-50 dark:bg-slate-900/60 border border-slate-200 dark:border-slate-800 text-[11px] text-slate-500 dark:text-slate-400">
                  Recursive permission changes are available only for local storage.
                </div>
              </div>
            </div>
          </template>
        </div>

        <div class="h-16 bg-gray-50/80 dark:bg-slate-900/80 border-t border-gray-200/80 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
          <button type="button" @click="closeModal" class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium cursor-pointer">Close</button>
          <div class="flex items-center space-x-2">
            <button v-if="activeTab === 'permissions'" type="button" :disabled="savingPerms" @click="handleSavePermissions" class="px-5 py-2.5 bg-blue-600 hover:bg-blue-700 active:scale-95 text-white font-semibold rounded-xl transition shadow-xs cursor-pointer flex items-center space-x-1.5 disabled:opacity-50">
              <span v-if="savingPerms" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-white border-t-transparent"></span><FbIcon v-else name="check" size="14px" /><span>{{ savingPerms ? 'Applying...' : 'Save Permissions' }}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { getMetadataApi, chmodFileApi } from '../../api/files';
import { normalizeApiError } from '../../utils/errorNormalizer';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useOverlayStore } from '../../overlays/overlayStore';
import { formatBytes, formatDate } from '../../utils/formatters';
import { supportsRecursiveChmod } from '../../domain/capabilities';
import type { IconName } from '../../utils/icons';

type PermissionScope = 'user' | 'group' | 'other';
const props = defineProps<{ modelValue: boolean; connectionId: string; path: string }>();
const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>();
const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();
const overlayStore = useOverlayStore();
const isOpen = ref(props.modelValue);
const activeTab = ref<'general' | 'permissions'>('general');
const meta = ref<any | null>(null);
const loading = ref(false);
const errorMsg = ref<string | null>(null);
const savingPerms = ref(false);
const applyRecursive = ref(false);
const isPathCopied = ref(false);
const isEtagCopied = ref(false);
const octalMode = ref('0755');
const permState = ref({ user: { r: true, w: true, x: true }, group: { r: true, w: false, x: true }, other: { r: true, w: false, x: true } });
const permissionScopes: Array<{ key: PermissionScope; label: string }> = [{ key: 'user', label: 'Owner (User)' }, { key: 'group', label: 'Group' }, { key: 'other', label: 'Others (Public)' }];
const canApplyRecursive = computed(() => supportsRecursiveChmod(props.connectionId));
const displayName = computed(() => meta.value?.name || (props.path ? props.path.split('/').pop() || 'Properties' : 'Properties'));
const itemVisual = computed(() => {
  const m = meta.value; const name = displayName.value; const isDir = m?.kind === 'directory';
  if (isDir) return { icon: 'folder' as IconName, gradient: 'from-amber-500/20 to-orange-500/20 text-amber-500 border-amber-500/30', badge: 'Folder', badgeColor: 'bg-amber-50 dark:bg-amber-950/50 text-amber-600 dark:text-amber-400 border-amber-200 dark:border-amber-800/60' };
  const ext = name.includes('.') ? name.split('.').pop()?.toLowerCase() : '';
  if (['jpg','jpeg','png','gif','webp','svg','bmp'].includes(ext || '')) return { icon: 'image' as IconName, gradient: 'from-purple-500/20 to-indigo-500/20 text-purple-500 border-purple-500/30', badge: `${ext?.toUpperCase()} Image`, badgeColor: 'bg-purple-50 dark:bg-purple-950/50 text-purple-600 dark:text-purple-400 border-purple-200 dark:border-purple-800/60' };
  if (['mp4','mkv','webm','mov','avi'].includes(ext || '')) return { icon: 'video' as IconName, gradient: 'from-rose-500/20 to-pink-500/20 text-rose-500 border-rose-500/30', badge: `${ext?.toUpperCase()} Video`, badgeColor: 'bg-rose-50 dark:bg-rose-950/50 text-rose-600 dark:text-rose-400 border-rose-200 dark:border-rose-800/60' };
  if (['mp3','wav','ogg','flac','aac','m4a'].includes(ext || '')) return { icon: 'audio' as IconName, gradient: 'from-emerald-500/20 to-teal-500/20 text-emerald-500 border-emerald-500/30', badge: `${ext?.toUpperCase()} Audio`, badgeColor: 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-600 dark:text-emerald-400 border-emerald-200 dark:border-emerald-800/60' };
  if (ext === 'pdf') return { icon: 'pdf' as IconName, gradient: 'from-red-500/20 to-rose-500/20 text-red-500 border-red-500/30', badge: 'PDF Document', badgeColor: 'bg-red-50 dark:bg-red-950/50 text-red-600 dark:text-red-400 border-red-200 dark:border-red-800/60' };
  if (['zip','tar','gz','bz2','xz','7z','rar'].includes(ext || '')) return { icon: 'archive' as IconName, gradient: 'from-amber-600/20 to-yellow-600/20 text-amber-600 border-amber-600/30', badge: `${ext?.toUpperCase()} Archive`, badgeColor: 'bg-amber-50 dark:bg-amber-950/50 text-amber-600 dark:text-amber-400 border-amber-200 dark:border-amber-800/60' };
  if (['js','ts','jsx','tsx','vue','html','css','json','py','rs','go','c','cpp','sh','sql','md'].includes(ext || '')) return { icon: 'code' as IconName, gradient: 'from-blue-500/20 to-cyan-500/20 text-blue-500 border-blue-500/30', badge: `${ext?.toUpperCase()} Code`, badgeColor: 'bg-blue-50 dark:bg-blue-950/50 text-blue-600 dark:text-blue-300 border-blue-200 dark:border-blue-800/60' };
  return { icon: 'file' as IconName, gradient: 'from-slate-500/20 to-gray-500/20 text-slate-500 border-slate-500/30', badge: ext ? `${ext.toUpperCase()} File` : 'File', badgeColor: 'bg-slate-50 dark:bg-slate-800 text-slate-600 dark:text-slate-300 border-slate-200 dark:border-slate-700' };
});
const permissionString = computed(() => {
  const p = permState.value; const lead = meta.value?.kind === 'directory' ? 'd' : (meta.value?.symlink_target ? 'l' : '-');
  return `${lead}${p.user.r?'r':'-'}${p.user.w?'w':'-'}${p.user.x?'x':'-'}${p.group.r?'r':'-'}${p.group.w?'w':'-'}${p.group.x?'x':'-'}${p.other.r?'r':'-'}${p.other.w?'w':'-'}${p.other.x?'x':'-'}`;
});
function closeModal() { isOpen.value = false; emit('update:modelValue', false); if (overlayStore.current?.type === 'properties') overlayStore.close(); }
function handleKeyDown(e: KeyboardEvent) { if (e.key === 'Escape' && isOpen.value) closeModal(); }
onMounted(() => window.addEventListener('keydown', handleKeyDown));
onUnmounted(() => window.removeEventListener('keydown', handleKeyDown));
watch(() => [props.modelValue, props.path, props.connectionId] as const, ([val, path, conn]) => {
  isOpen.value = !!val;
  if (!supportsRecursiveChmod(conn)) applyRecursive.value = false;
  if (val && path && conn) void fetchMetadata();
}, { immediate: true });
watch(() => isOpen.value, (val) => emit('update:modelValue', val));
async function fetchMetadata() {
  if (!props.path || !props.connectionId) return;
  loading.value = true; errorMsg.value = null;
  try {
    meta.value = await getMetadataApi(props.connectionId, props.path);
    if (meta.value.permissions) parsePermissionsString(meta.value.permissions); else { octalMode.value = meta.value.kind === 'directory' ? '0755' : '0644'; parseOctal(octalMode.value); }
  } catch (err: unknown) { const norm = normalizeApiError(err); errorMsg.value = norm.message || 'Failed to load properties'; uiStore.showToast(errorMsg.value, 'error'); } finally { loading.value = false; }
}
function parsePermissionsString(p: string) { const clean = p.length === 10 ? p.substring(1) : p; if (clean.length === 9) { permState.value = { user: { r: clean[0] === 'r', w: clean[1] === 'w', x: clean[2] === 'x' }, group: { r: clean[3] === 'r', w: clean[4] === 'w', x: clean[5] === 'x' }, other: { r: clean[6] === 'r', w: clean[7] === 'w', x: clean[8] === 'x' } }; recalcOctal(); } }
function parseOctal(oct: string) { const clean = oct.replace(/^0+/, '').padStart(3, '0'); const u = parseInt(clean[0] || '0', 10); const g = parseInt(clean[1] || '0', 10); const o = parseInt(clean[2] || '0', 10); permState.value = { user: { r: (u&4)!==0, w: (u&2)!==0, x: (u&1)!==0 }, group: { r: (g&4)!==0, w: (g&2)!==0, x: (g&1)!==0 }, other: { r: (o&4)!==0, w: (o&2)!==0, x: (o&1)!==0 } }; }
function onOctalInput() { parseOctal(octalMode.value); }
function applyPreset(oct: string) { octalMode.value = oct; parseOctal(oct); }
function recalcOctal() { const u=(permState.value.user.r?4:0)+(permState.value.user.w?2:0)+(permState.value.user.x?1:0); const g=(permState.value.group.r?4:0)+(permState.value.group.w?2:0)+(permState.value.group.x?1:0); const o=(permState.value.other.r?4:0)+(permState.value.other.w?2:0)+(permState.value.other.x?1:0); octalMode.value=`0${u}${g}${o}`; }
async function handleSavePermissions() {
  savingPerms.value = true;
  try {
    const modeInt = parseInt(octalMode.value, 8);
    await chmodFileApi(props.connectionId, { path: props.path, mode: modeInt, recursive: canApplyRecursive.value && applyRecursive.value });
    uiStore.showToast('Permissions updated successfully!', 'success'); await fetchMetadata(); await workspaceStore.refreshAll();
  } catch (err: unknown) { uiStore.showToast(normalizeApiError(err).message, 'error'); } finally { savingPerms.value = false; }
}
function copyPath(p: string) { navigator.clipboard.writeText(p); isPathCopied.value = true; setTimeout(() => { isPathCopied.value = false; }, 2000); uiStore.showToast('Path copied to clipboard', 'success'); }
function copyEtag(etag: string) { navigator.clipboard.writeText(etag); isEtagCopied.value = true; setTimeout(() => { isEtagCopied.value = false; }, 2000); uiStore.showToast('ETag copied to clipboard', 'success'); }
</script>
