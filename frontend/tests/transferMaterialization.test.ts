import { describe, expect, test } from 'bun:test';
import { createPinia, setActivePinia } from 'pinia';
import { materializeTransferJob, useTransferStore } from '../src/stores/transferStore';
import type { TransferJob } from '../src/types/transfer';

function makeJob(id: string, createdAt: string, transferred = 0): TransferJob {
  return {
    id,
    name: id,
    transfer_type: 'upload',
    source_connection_id: 'upload',
    source_path: `upload://${id}`,
    destination_connection_id: 'local',
    destination_path: `/${id}`,
    status: 'running',
    phase: 'transferring',
    execution_mode: 'inline',
    staging: 'none',
    transferred_bytes: transferred,
    total_bytes: 100,
    speed_bytes_per_sec: 0,
    created_at: createdAt,
    updated_at: createdAt,
  };
}

describe('realtime transfer materialization', () => {
  test('adds a newly discovered job with newest jobs first', () => {
    const older = makeJob('older', '2026-01-01T00:00:00Z');
    const newer = makeJob('newer', '2026-01-02T00:00:00Z');

    expect(materializeTransferJob([older], newer).map((job) => job.id)).toEqual([
      'newer',
      'older',
    ]);
  });

  test('does not duplicate or overwrite an existing REST snapshot', () => {
    const rest = makeJob('same', '2026-01-01T00:00:00Z', 25);
    const current = [rest];
    const tick = makeJob('same', '2026-01-01T00:00:00Z', 75);
    const result = materializeTransferJob(current, tick);

    expect(result).toBe(current);
    expect(result).toHaveLength(1);
    expect(result[0].transferred_bytes).toBe(25);
  });

  test('late session registration never resets visible client progress', () => {
    setActivePinia(createPinia());
    const store = useTransferStore();
    const registration = {
      id: 'upload-1',
      name: 'large.bin',
      transferType: 'upload' as const,
      totalBytes: 100,
      sourceConnectionId: 'upload',
      sourcePath: 'upload://upload-1',
      destinationConnectionId: 'local',
      destinationPath: '/large.bin',
    };

    store.registerLiveTransfer(registration);
    store.updateClientProgress('upload-1', 60, 100);
    store.registerLiveTransfer(registration);

    expect(store.liveProgress['upload-1'].transferredBytes).toBe(60);
    expect(store.liveProgress['upload-1'].totalBytes).toBe(100);
  });
});
