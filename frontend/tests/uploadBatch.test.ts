import { describe, expect, it, beforeEach } from 'bun:test';
import { createPinia, setActivePinia } from 'pinia';
import { useTransferStore, generateUniqueName } from '../src/stores/transferStore';

describe('uploadBatch and unique name helper', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('generates unique name by incrementing counter when conflict exists', () => {
    const existing = new Set(['photo.jpg', 'photo (1).jpg', 'document.pdf', 'Makefile']);

    expect(generateUniqueName('photo.jpg', existing)).toBe('photo (2).jpg');
    expect(generateUniqueName('document.pdf', existing)).toBe('document (1).pdf');
    expect(generateUniqueName('Makefile', existing)).toBe('Makefile (1)');
    expect(generateUniqueName('new.txt', existing)).toBe('new (1).txt');
  });

  it('opens drawer and registers queued jobs immediately on submitUploadBatch', async () => {
    const store = useTransferStore();
    expect(store.isDrawerOpen).toBe(false);

    const file1 = new File(['content1'], 'file1.txt', { type: 'text/plain' });
    const file2 = new File(['content2'], 'file2.txt', { type: 'text/plain' });

    // Mock uploader so it does not make real network requests in this unit test
    let uploadCallCount = 0;
    const mockUploader = async (connId: string, targetDir: string, file: File, signal: AbortSignal, onProgress?: any, onSession?: any) => {
      uploadCallCount++;
      const jobId = `job-${uploadCallCount}`;
      onSession?.({
        session_id: `session-${uploadCallCount}`,
        job_id: jobId,
        upload_url: `/api/v1/connections/${connId}/uploads/${jobId}`,
        expires_at: '2026-01-01T00:00:00Z',
      });
      return {
        session_id: `session-${uploadCallCount}`,
        job_id: jobId,
        upload_url: `/api/v1/connections/${connId}/uploads/${jobId}`,
        expires_at: '2026-01-01T00:00:00Z',
      };
    };

    const batchPromise = store.submitUploadBatch({
      connectionId: 'local',
      targetDir: '/docs',
      files: [file1, file2],
      concurrency: 1,
      uploader: mockUploader,
    });

    // Drawer opens immediately
    expect(store.isDrawerOpen).toBe(true);

    const result = await batchPromise;
    expect(result.successfulCount).toBe(2);
    expect(result.cancelled).toBe(false);
    expect(uploadCallCount).toBe(2);
  });

  it('cancels queued upload cleanly when cancelTransfer is called on tempId', async () => {
    const store = useTransferStore();

    const file1 = new File(['1'], 'first.txt', { type: 'text/plain' });
    const file2 = new File(['2'], 'second.txt', { type: 'text/plain' });

    let startedSecond = false;
    const mockUploader = async (connId: string, targetDir: string, file: File, signal: AbortSignal, onProgress?: any, onSession?: any) => {
      if (file.name === 'second.txt') {
        startedSecond = true;
      }
      return {
        session_id: 's1',
        job_id: 'j1',
        upload_url: '/upload',
        expires_at: '2026-01-01T00:00:00Z',
      };
    };

    // Call submit with concurrency 1
    const p = store.submitUploadBatch({
      connectionId: 'local',
      targetDir: '/',
      files: [file1, file2],
      concurrency: 1,
      uploader: mockUploader,
    });

    // Check that items are queued
    const queuedIds = Object.keys(store.liveProgress);
    expect(queuedIds.length).toBeGreaterThan(0);

    const secondJobEntry = Object.entries(store.liveProgress).find(
      ([_, p]) => p.name === 'second.txt'
    );
    if (secondJobEntry) {
      const interceptedSecondId = secondJobEntry[0];
      await store.cancelTransfer(interceptedSecondId);
    }

    const res = await p;
    expect(res.successfulCount).toBe(1);
    expect(startedSecond).toBe(false);
  });
});
