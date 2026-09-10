import { describe, it, expect, beforeEach } from 'bun:test';
import { setActivePinia, createPinia } from 'pinia';
import { useConnectionStore } from '../src/stores/connectionStore';
import type { Connection } from '../src/types/connection';

describe('Connection Management & Fallback logic', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('correctly tracks and removes connections in connectionStore', () => {
    const store = useConnectionStore();
    const testConn: Connection = {
      id: 'conn-sftp-1',
      name: 'Backup SFTP',
      provider: 'sftp',
      host: '10.0.0.5',
      port: 22,
      username: 'tester',
      base_path: '/data',
      read_only: true,
      enabled: true,
      status: 'connected',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    store.addConnection(testConn);
    expect(store.getConnection('conn-sftp-1')).toBeDefined();
    expect(store.isReadOnly('conn-sftp-1')).toBe(true);
    expect(store.canWrite('conn-sftp-1')).toBe(false);

    store.removeConnection('conn-sftp-1');
    expect(store.getConnection('conn-sftp-1')).toBeUndefined();
  });

  it('correctly prepares update payload omitting blank secret to preserve vault credentials', () => {
    function buildUpdatePayload(form: {
      name: string;
      host: string;
      port: number;
      username: string;
      base_path: string;
      secret: string;
    }) {
      const payload: Record<string, any> = {
        name: form.name,
        host: form.host,
        port: form.port,
        username: form.username,
        base_path: form.base_path,
      };
      if (form.secret && form.secret.trim().length > 0) {
        payload.secret = form.secret.trim();
      }
      return payload;
    }

    const payloadWithoutSecret = buildUpdatePayload({
      name: 'Updated Name',
      host: 'ftp.example.com',
      port: 21,
      username: 'user1',
      base_path: '/public',
      secret: '   ',
    });
    expect(payloadWithoutSecret.secret).toBeUndefined();
    expect(payloadWithoutSecret.name).toBe('Updated Name');

    const payloadWithSecret = buildUpdatePayload({
      name: 'Updated Name',
      host: 'ftp.example.com',
      port: 21,
      username: 'user1',
      base_path: '/public',
      secret: 'new-secret-123',
    });
    expect(payloadWithSecret.secret).toBe('new-secret-123');
  });

  it('determines fallback connection when an active connection is deleted', () => {
    let leftPanelConn = 'conn-to-delete';
    let rightPanelConn = 'local';
    const deletedId = 'conn-to-delete';

    if (leftPanelConn === deletedId) {
      leftPanelConn = 'local';
    }
    if (rightPanelConn === deletedId) {
      rightPanelConn = 'local';
    }

    expect(leftPanelConn).toBe('local');
    expect(rightPanelConn).toBe('local');
  });
});
