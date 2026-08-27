import { defineStore } from 'pinia';
import { ref } from 'vue';
import { listConnectionsApi } from '../api/connections';
import type { Connection } from '../types/connection';

export const useConnectionStore = defineStore('connection', () => {
  const connections = ref<Connection[]>([
    {
      id: 'local',
      name: 'Local Storage',
      provider: 'local',
      base_path: '/',
      read_only: false,
      enabled: true,
      status: 'connected',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    },
  ]);

  const activeConnectionId = ref<string>('local');

  async function fetchConnections() {
    try {
      const data = await listConnectionsApi();
      if (data && data.length > 0) {
        connections.value = data;
      }
    } catch (err) {
      console.error('Failed to fetch connections', err);
    }
  }

  function getConnection(id: string): Connection | undefined {
    return connections.value.find((c) => c.id === id);
  }

  function addConnection(conn: Connection) {
    connections.value.push(conn);
  }

  function removeConnection(id: string) {
    connections.value = connections.value.filter((c) => c.id !== id);
  }

  return {
    connections,
    activeConnectionId,
    fetchConnections,
    getConnection,
    addConnection,
    removeConnection,
  };
});
