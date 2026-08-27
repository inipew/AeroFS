import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { apiClient } from '../api/client';
import { useWorkspaceStore } from './workspaceStore';
import { useUiStore } from './uiStore';

export type Operation =
  | {
      type: 'rename';
      description: string;
      connectionId: string;
      oldPath: string;
      newPath: string;
    }
  | {
      type: 'move';
      description: string;
      fromConnectionId: string;
      toConnectionId: string;
      sourcePath: string;
      destPath: string;
    }
  | {
      type: 'create';
      description: string;
      connectionId: string;
      path: string;
      kind: 'file' | 'directory';
    }
  | {
      type: 'trash';
      description: string;
      connectionId: string;
      trashItemId: string;
      originalPath: string;
    };

export const useHistoryStore = defineStore('history', () => {
  const undoStack = ref<Operation[]>([]);
  const redoStack = ref<Operation[]>([]);
  const isExecuting = ref(false);

  const canUndo = computed(() => undoStack.value.length > 0 && !isExecuting.value);
  const canRedo = computed(() => redoStack.value.length > 0 && !isExecuting.value);

  function pushOperation(op: Operation) {
    undoStack.value.push(op);
    redoStack.value = []; // Clear redo stack on new operation

    const uiStore = useUiStore();
    uiStore.showToast(`Operation: ${op.description}`, 'info');
  }

  async function undo() {
    if (!canUndo.value) return;
    const op = undoStack.value.pop();
    if (!op) return;

    isExecuting.value = true;
    const workspaceStore = useWorkspaceStore();
    const uiStore = useUiStore();

    try {
      switch (op.type) {
        case 'rename': {
          await apiClient.post(`/connections/${op.connectionId}/files/rename`, {
            source_path: op.newPath,
            destination_path: op.oldPath,
          });
          uiStore.showToast(`Undid rename: ${op.newPath.split('/').pop()} → ${op.oldPath.split('/').pop()}`, 'success');
          break;
        }
        case 'move': {
          await apiClient.post(`/connections/${op.toConnectionId}/files/move`, {
            source_path: op.destPath,
            destination_path: op.sourcePath,
          });
          uiStore.showToast(`Undid move to ${op.sourcePath}`, 'success');
          break;
        }
        case 'create': {
          await apiClient.delete(`/connections/${op.connectionId}/files`, {
            params: { path: op.path },
          });
          uiStore.showToast(`Undid creation of ${op.path.split('/').pop()}`, 'success');
          break;
        }
        case 'trash': {
          await apiClient.post(`/trash/restore/${op.trashItemId}`);
          uiStore.showToast(`Restored ${op.originalPath.split('/').pop()} from trash`, 'success');
          break;
        }
      }

      redoStack.value.push(op);
      await workspaceStore.refreshAll();
    } catch (err: any) {
      uiStore.showToast(err.response?.data?.error?.message || 'Undo operation failed', 'error');
      undoStack.value.push(op); // Re-push on failure
    } finally {
      isExecuting.value = false;
    }
  }

  async function redo() {
    if (!canRedo.value) return;
    const op = redoStack.value.pop();
    if (!op) return;

    isExecuting.value = true;
    const workspaceStore = useWorkspaceStore();
    const uiStore = useUiStore();

    try {
      switch (op.type) {
        case 'rename': {
          await apiClient.post(`/connections/${op.connectionId}/files/rename`, {
            source_path: op.oldPath,
            destination_path: op.newPath,
          });
          uiStore.showToast(`Redid rename to ${op.newPath.split('/').pop()}`, 'success');
          break;
        }
        case 'move': {
          await apiClient.post(`/connections/${op.fromConnectionId}/files/move`, {
            source_path: op.sourcePath,
            destination_path: op.destPath,
          });
          uiStore.showToast(`Redid move to ${op.destPath}`, 'success');
          break;
        }
        case 'create': {
          if (op.kind === 'directory') {
            await apiClient.post(`/connections/${op.connectionId}/directories`, { path: op.path });
          } else {
            await apiClient.post(`/connections/${op.connectionId}/files`, { path: op.path, content: '' });
          }
          uiStore.showToast(`Redid creation of ${op.path.split('/').pop()}`, 'success');
          break;
        }
        case 'trash': {
          uiStore.showToast('Redo for trash deletion is not supported', 'info');
          break;
        }
      }

      undoStack.value.push(op);
      await workspaceStore.refreshAll();
    } catch (err: any) {
      uiStore.showToast(err.response?.data?.error?.message || 'Redo operation failed', 'error');
      redoStack.value.push(op);
    } finally {
      isExecuting.value = false;
    }
  }

  return {
    undoStack,
    redoStack,
    canUndo,
    canRedo,
    pushOperation,
    undo,
    redo,
  };
});
