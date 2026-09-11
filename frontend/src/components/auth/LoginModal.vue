<template>
  <Transition name="login-page-fade" appear>
    <div
      class="fixed inset-0 z-50 overflow-y-auto flex flex-col justify-between bg-slate-50 dark:bg-[#090d16] text-slate-800 dark:text-slate-100 font-sans transition-colors duration-200 select-none p-4 sm:p-6"
    >
      <!-- Subtle Ambient Glow -->
      <div
        class="fixed top-0 left-1/2 -translate-x-1/2 w-[600px] h-[300px] bg-blue-500/5 dark:bg-blue-500/10 blur-3xl pointer-events-none"
      ></div>

      <!-- Header: Theme Switcher Only -->
      <header class="w-full max-w-sm mx-auto flex items-center justify-end pt-2">
        <button
          type="button"
          @click="themeStore.toggleTheme()"
          class="p-2 rounded-xl text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 hover:bg-slate-200/60 dark:hover:bg-slate-800/60 transition-colors cursor-pointer focus:outline-none"
          :title="themeStore.isDark ? 'Switch to Light Mode' : 'Switch to Dark Mode'"
          aria-label="Toggle theme"
        >
          <Sun v-if="themeStore.isDark" class="w-4 h-4 text-amber-400" />
          <Moon v-else class="w-4 h-4 text-slate-600" />
        </button>
      </header>

      <!-- Center: Minimalist Login Card -->
      <main class="w-full max-w-[380px] mx-auto my-auto">
        <div
          class="bg-white dark:bg-slate-900 border border-slate-200/90 dark:border-slate-800 rounded-2xl p-7 sm:p-8 shadow-xl shadow-slate-200/50 dark:shadow-2xl dark:shadow-black/50"
          :class="{ 'card-shake': hasErrorShake }"
        >
          <!-- Brand Icon & Header -->
          <div class="flex flex-col items-center text-center mb-6">
            <div
              class="w-11 h-11 rounded-xl bg-gradient-to-tr from-blue-600 to-indigo-500 flex items-center justify-center text-white shadow-md shadow-blue-500/20 mb-3.5"
            >
              <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path>
              </svg>
            </div>
            <h1 class="text-xl font-bold tracking-tight text-slate-900 dark:text-white">
              Sign in to AeroFS
            </h1>
            <p class="text-xs text-slate-500 dark:text-slate-400 mt-1">
              Multi-Backend Storage Gateway
            </p>
          </div>

          <!-- Error Alert Banner -->
          <Transition name="fade">
            <div
              v-if="authStore.error"
              class="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-red-600 dark:text-red-400 text-xs flex items-center justify-between gap-2"
            >
              <div class="flex items-center gap-2">
                <AlertCircle class="w-4 h-4 shrink-0 text-red-500" />
                <span class="font-medium leading-snug">{{ authStore.error }}</span>
              </div>
              <button
                type="button"
                @click="authStore.error = null"
                class="text-red-500 hover:text-red-700 dark:hover:text-red-300 p-0.5 cursor-pointer"
                aria-label="Dismiss error"
              >
                <X class="w-3.5 h-3.5" />
              </button>
            </div>
          </Transition>

          <!-- Login Form -->
          <form @submit.prevent="handleLogin" class="space-y-4">
            <!-- Username Input -->
            <div>
              <label for="login-username" class="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                Username
              </label>
              <div class="relative flex items-center">
                <div class="absolute left-3 text-slate-400 dark:text-slate-500 pointer-events-none">
                  <User class="w-4 h-4" />
                </div>
                <input
                  id="login-username"
                  ref="usernameInputRef"
                  v-model="username"
                  type="text"
                  autocomplete="username"
                  placeholder="Username"
                  required
                  class="w-full bg-slate-50 dark:bg-slate-950/60 border border-slate-200 dark:border-slate-800 rounded-xl pl-9 pr-8 py-2 text-xs sm:text-sm text-slate-900 dark:text-white placeholder-slate-400 dark:placeholder-slate-500 font-medium focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-colors"
                />
                <button
                  v-if="username"
                  type="button"
                  @click="username = ''"
                  class="absolute right-2.5 p-0.5 rounded-full text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 cursor-pointer"
                  aria-label="Clear username"
                >
                  <X class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            <!-- Password Input -->
            <div>
              <div class="flex items-center justify-between mb-1.5">
                <label for="login-password" class="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                  Password
                </label>
                <!-- Caps Lock Warning -->
                <span
                  v-if="isCapsLockOn"
                  class="text-[11px] font-medium text-amber-600 dark:text-amber-400 flex items-center gap-1"
                >
                  <AlertCircle class="w-3 h-3" />
                  Caps Lock ON
                </span>
              </div>

              <div class="relative flex items-center">
                <div class="absolute left-3 text-slate-400 dark:text-slate-500 pointer-events-none">
                  <Lock class="w-4 h-4" />
                </div>
                <input
                  id="login-password"
                  v-model="password"
                  :type="showPassword ? 'text' : 'password'"
                  autocomplete="current-password"
                  placeholder="••••••••"
                  required
                  @keydown="checkCapsLock"
                  @keyup="checkCapsLock"
                  @blur="isCapsLockOn = false"
                  class="w-full bg-slate-50 dark:bg-slate-950/60 border border-slate-200 dark:border-slate-800 rounded-xl pl-9 pr-9 py-2 text-xs sm:text-sm text-slate-900 dark:text-white placeholder-slate-400 dark:placeholder-slate-500 font-medium focus:outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 transition-colors"
                />
                <button
                  type="button"
                  @click="showPassword = !showPassword"
                  class="absolute right-2.5 p-1 rounded-lg text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 cursor-pointer transition-colors"
                  :title="showPassword ? 'Hide password' : 'Show password'"
                  aria-label="Toggle password visibility"
                >
                  <EyeOff v-if="showPassword" class="w-4 h-4" />
                  <Eye v-else class="w-4 h-4" />
                </button>
              </div>
            </div>

            <!-- Remember me & Demo shortcut -->
            <div class="flex items-center justify-between pt-0.5">
              <label class="flex items-center gap-2 cursor-pointer select-none">
                <input
                  v-model="rememberUsername"
                  type="checkbox"
                  class="w-3.5 h-3.5 rounded text-blue-600 border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 focus:ring-blue-500 focus:ring-offset-0 cursor-pointer"
                />
                <span class="text-xs text-slate-500 dark:text-slate-400">Remember me</span>
              </label>

              <button
                type="button"
                @click="fillDemoCredentials"
                class="text-xs font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
              >
                {{ demoFilledNotice ? 'Filled!' : 'Fill demo' }}
              </button>
            </div>

            <!-- Submit Button -->
            <button
              type="submit"
              :disabled="loading"
              class="w-full mt-1 py-2.5 px-4 rounded-xl bg-blue-600 hover:bg-blue-500 active:scale-[0.99] text-white font-semibold shadow-md shadow-blue-500/20 transition duration-150 disabled:opacity-60 disabled:cursor-not-allowed text-xs sm:text-sm flex items-center justify-center gap-2 cursor-pointer"
            >
              <span
                v-if="loading"
                class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-white border-t-transparent"
              ></span>
              <span>{{ loading ? 'Signing in...' : 'Sign In' }}</span>
            </button>
          </form>

          <!-- Minimalist Demo Hint -->
          <div class="mt-5 pt-4 border-t border-slate-100 dark:border-slate-800/80 text-center">
            <p class="text-[11px] text-slate-400 dark:text-slate-500">
              Demo credentials:
              <button
                type="button"
                @click="fillDemoCredentials"
                class="font-mono text-slate-600 dark:text-slate-300 hover:text-blue-500 transition-colors cursor-pointer"
              >
                admin / admin12345
              </button>
            </p>
          </div>
        </div>
      </main>

      <!-- Minimal Footer -->
      <footer class="w-full max-w-sm mx-auto text-center pb-2 text-[11px] text-slate-400 dark:text-slate-600">
        <span>AeroFS Gateway</span>
      </footer>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import {
  User,
  Lock,
  Eye,
  EyeOff,
  Sun,
  Moon,
  AlertCircle,
  X,
} from 'lucide-vue-next';
import { useAuthStore } from '../../stores/authStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useTransferStore } from '../../stores/transferStore';
import { useThemeStore } from '../../stores/themeStore';

const REMEMBERED_USERNAME_KEY = 'fb:remembered_username';

const authStore = useAuthStore();
const workspaceStore = useWorkspaceStore();
const transferStore = useTransferStore();
const themeStore = useThemeStore();

const usernameInputRef = ref<HTMLInputElement | null>(null);

const storedUsername = localStorage.getItem(REMEMBERED_USERNAME_KEY);
const username = ref(storedUsername || 'admin');
const password = ref(storedUsername ? '' : 'admin12345');
const rememberUsername = ref(Boolean(storedUsername) || true);

const showPassword = ref(false);
const isCapsLockOn = ref(false);
const demoFilledNotice = ref(false);
const hasErrorShake = ref(false);
const loading = ref(false);

function checkCapsLock(e: KeyboardEvent) {
  if (typeof e.getModifierState === 'function') {
    isCapsLockOn.value = e.getModifierState('CapsLock');
  }
}

function fillDemoCredentials() {
  username.value = 'admin';
  password.value = 'admin12345';
  demoFilledNotice.value = true;
  authStore.error = null;
  setTimeout(() => {
    demoFilledNotice.value = false;
  }, 1500);
}

function triggerShake() {
  hasErrorShake.value = true;
  setTimeout(() => {
    hasErrorShake.value = false;
  }, 600);
}

async function handleLogin() {
  loading.value = true;
  authStore.error = null;

  try {
    const ok = await authStore.login({
      username: username.value,
      password: password.value,
    });

    if (ok) {
      if (rememberUsername.value) {
        localStorage.setItem(REMEMBERED_USERNAME_KEY, username.value);
      } else {
        localStorage.removeItem(REMEMBERED_USERNAME_KEY);
      }

      transferStore.connectWs();
      await transferStore.fetchJobs();
      await workspaceStore.refreshPanel('left');
      if (workspaceStore.isDualPane) {
        await workspaceStore.refreshPanel('right');
      }
    } else {
      triggerShake();
    }
  } catch {
    triggerShake();
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  if (username.value && password.value) {
    // Already populated
  } else if (username.value && !password.value) {
    const pwdInput = document.getElementById('login-password');
    pwdInput?.focus();
  } else {
    usernameInputRef.value?.focus();
  }
});
</script>

<style scoped>
.login-page-fade-enter-active,
.login-page-fade-leave-active {
  transition: opacity 200ms ease;
}
.login-page-fade-enter-from,
.login-page-fade-leave-to {
  opacity: 0;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 150ms ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@keyframes card-shake-anim {
  0%, 100% {
    transform: translateX(0);
  }
  20%, 60% {
    transform: translateX(-5px);
  }
  40%, 80% {
    transform: translateX(5px);
  }
}

.card-shake {
  animation: card-shake-anim 0.5s ease-in-out both;
}
</style>

