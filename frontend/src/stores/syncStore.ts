import { defineStore } from 'pinia';
import { computed } from 'vue';
import { queryClient } from '../queryClient';
import { queryKeys } from '../api/queryKeys';
import { listSyncJobsApi } from '../api/sync';
import type { SyncJob } from '../types/sync';

export const useSyncStore = defineStore('sync', () => {
  const jobs = computed<SyncJob[]>(() => {
    return queryClient.getQueryData<SyncJob[]>(queryKeys.syncJobs()) ?? [];
  });

  const activeJobs = computed(() =>
    jobs.value.filter((j) =>
      ['scanning', 'planning', 'reconciling', 'executing'].includes(j.status)
    )
  );

  const conflictJobs = computed(() =>
    jobs.value.filter((j) => j.status === 'conflict')
  );

  async function fetchJobs() {
    return await queryClient.fetchQuery({
      queryKey: queryKeys.syncJobs(),
      queryFn: () => listSyncJobsApi(),
    });
  }

  function handleSyncJobUpdate(job: SyncJob) {
    queryClient.setQueryData<SyncJob[]>(queryKeys.syncJobs(), (old) => {
      const current = old ?? [];
      const idx = current.findIndex((j) => j.id === job.id);
      if (idx >= 0) {
        const next = [...current];
        next[idx] = job;
        return next;
      }
      return [job, ...current];
    });
  }

  return {
    jobs,
    activeJobs,
    conflictJobs,
    fetchJobs,
    handleSyncJobUpdate,
  };
});
