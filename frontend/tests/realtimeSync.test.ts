import { describe, expect, it } from 'bun:test';
import { RealtimeSyncCoordinator } from '../src/services/realtimeSync';

describe('RealtimeSyncCoordinator', () => {
  it('coalesces multiple invalidations on the same directory', () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/documents');
    coordinator.queueDirectoryInvalidation('conn-1', '/documents');
    coordinator.queueDirectoryInvalidation('conn-1', '/documents/');

    expect(coordinator.getPendingCount()).toBe(1);
    coordinator.stop();
  });

  it('keeps distinct directories and connections separated in queue', () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/documents');
    coordinator.queueDirectoryInvalidation('conn-1', '/photos');
    coordinator.queueDirectoryInvalidation('conn-2', '/documents');

    expect(coordinator.getPendingCount()).toBe(3);
    coordinator.stop();
  });

  it('flushes pending directories cleanly', () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/downloads');
    expect(coordinator.getPendingCount()).toBe(1);

    coordinator.flush();
    expect(coordinator.getPendingCount()).toBe(0);
    coordinator.stop();
  });

  it('automatically flushes after debounce interval', async () => {
    const coordinator = new RealtimeSyncCoordinator();

    coordinator.queueDirectoryInvalidation('conn-1', '/music');
    expect(coordinator.getPendingCount()).toBe(1);

    // Wait slightly longer than 150ms debounce
    await new Promise((resolve) => setTimeout(resolve, 200));

    expect(coordinator.getPendingCount()).toBe(0);
    coordinator.stop();
  });
});
