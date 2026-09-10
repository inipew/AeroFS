import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { restoreTrashItem } from '../api/trash';
import { useWorkspaceStore } from './workspaceStore';
import { useUiStore } from './uiStore';
import { renameEntryApi, createFileApi, createDirectoryApi, deleteFilesApi } from '../api/files';
import { createTransferApi } from '../api/transfers';

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
          await renameEntryApi(op.connectionId, op.newPath, op.oldPath);
          uiStore.showToast(`Undid rename: ${op.newPath.split('/').pop()} → ${op.oldPath.split('/').pop()}`, 'success');
          break;
        }
        case 'move': {
          if (op.fromConnectionId === op.toConnectionId) {
            await renameEntryApi(op.toConnectionId, op.destPath, op.sourcePath);
          } else {
            await createTransferApi({
              name: `Undo move ${op.destPath.split('/').pop()}`,
              transfer_type: 'move',
              source_connection_id: op.toConnectionId,
              source_path: op.destPath,
              destination_connection_id: op.fromConnectionId,
              destination_path: op.sourcePath.substring(0, op.sourcePath.lastIndexOf('/')) || '/',
            });
          }
          uiStore.showToast(`Undid move to ${op.sourcePath}`, 'success');
          break;
        }
        case 'create': {
          await deleteFilesApi(op.connectionId, [op.path]);
          uiStore.showToast(`Undid creation of ${op.path.split('/').pop()}`, 'success');
          break;
        }
        case 'trash': {
          await restoreTrashItem(op.trashItemId);
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
          await renameEntryApi(op.connectionId, op.oldPath, op.newPath);
          uiStore.showToast(`Redid rename to ${op.newPath.split('/').pop()}`, 'success');
          break;
        }
        case 'move': {
          if (op.fromConnectionId === op.toConnectionId) {
            await renameEntryApi(op.fromConnectionId, op.sourcePath, op.destPath);
          } else {
            await createTransferApi({
              name: `Redo move ${op.sourcePath.split('/').pop()}`,
              transfer_type: 'move',
              source_connection_id: op.fromConnectionId,
              source_path: op.sourcePath,
              destination_connection_id: op.toConnectionId,
              destination_path: op.destPath.substring(0, op.destPath.lastIndexOf('/')) || '/',
            });
          }
          uiStore.showToast(`Redid move to ${op.destPath}`, 'success');
          break;
        }
        case 'create': {
          if (op.kind === 'directory') {
            await createDirectoryApi(op.connectionId, op.path);
          } else {
            await createFileApi(op.connectionId, op.path);
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
