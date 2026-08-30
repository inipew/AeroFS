import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { listSyncJobsApi } from '../api/sync'
import type { SyncJob } from '../types/sync'

export const useSyncStore = defineStore('sync', () => {
  const jobs = ref<SyncJob[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)
  
  const activeJobs = computed(() => jobs.value.filter(j => 
    ['scanning', 'planning', 'reconciling', 'executing'].includes(j.status)
  ))
  
  const conflictJobs = computed(() => jobs.value.filter(j => j.status === 'conflict'))
  
  async function fetchJobs() {
    isLoading.value = true
    error.value = null
    try {
      jobs.value = await listSyncJobsApi()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to load sync jobs'
    } finally {
      isLoading.value = false
    }
  }
  
  function handleSyncJobUpdate(job: SyncJob) {
    const idx = jobs.value.findIndex(j => j.id === job.id)
    if (idx >= 0) {
      jobs.value[idx] = job
    } else {
      jobs.value.unshift(job)
    }
  }
  
  return { jobs, isLoading, error, activeJobs, conflictJobs, fetchJobs, handleSyncJobUpdate }
})
