import { describe, expect, it, beforeEach } from 'bun:test';
import { ref } from 'vue';
import { createPinia, setActivePinia } from 'pinia';
import { useFileDragDrop } from '../src/features/files/composables/useFileDragDrop';
import { useTransferStore } from '../src/stores/transferStore';
import type { FileEntry } from '../src/types/vfs';

describe('Drag and Drop Behavior & Lifecycle', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('handleDrop calls preventDefault and stopPropagation on the event', async () => {
    const currentPath = ref('/documents');
    const connectionId = ref('local');
    const entries = ref<FileEntry[]>([]);
    const selectedPaths = ref<string[]>([]);
    const containerRef = ref<HTMLElement | null>(null);

    const dnd = useFileDragDrop({
      panelId: 'left',
      connectionId,
      currentPath,
      entries,
      selectedPaths,
      containerRef,
      onSelect: () => {},
    });

    let prevented = false;
    let stopped = false;
    const mockEvent = {
      preventDefault: () => {
        prevented = true;
      },
      stopPropagation: () => {
        stopped = true;
      },
      dataTransfer: {
        getData: () => '',
        items: [],
        files: [],
      },
    } as unknown as DragEvent;

    await dnd.handleDrop(mockEvent);

    expect(prevented).toBe(true);
    expect(stopped).toBe(true);
    expect(dnd.isDragOver.value).toBe(false);
  });

  it('handleDrop with external files routes to submitUploadBatch with current path', async () => {
    const transferStore = useTransferStore();
    const currentPath = ref('/photos');
    const connectionId = ref('local');
    const entries = ref<FileEntry[]>([]);
    const selectedPaths = ref<string[]>([]);
    const containerRef = ref<HTMLElement | null>(null);

    let submittedBatch: any = null;
    transferStore.submitUploadBatch = (async (options: any) => {
      submittedBatch = options;
      return { successfulCount: 1, failedCount: 0, cancelled: false };
    }) as any;

    const dnd = useFileDragDrop({
      panelId: 'left',
      connectionId,
      currentPath,
      entries,
      selectedPaths,
      containerRef,
      onSelect: () => {},
    });

    const file = new File(['test content'], 'sample.png', { type: 'image/png' });
    const mockEvent = {
      preventDefault: () => {},
      stopPropagation: () => {},
      dataTransfer: {
        getData: () => '',
        items: [],
        files: [file],
      },
    } as unknown as DragEvent;

    await dnd.handleDrop(mockEvent);

    expect(submittedBatch).not.toBeNull();
    expect(submittedBatch.targetDir).toBe('/photos');
    expect(submittedBatch.connectionId).toBe('local');
    expect(submittedBatch.files.length).toBe(1);
    expect(submittedBatch.files[0].file.name).toBe('sample.png');
  });

  it('handleDrop targeting a subfolder routes upload to that subfolder', async () => {
    const transferStore = useTransferStore();
    const currentPath = ref('/documents');
    const connectionId = ref('local');
    const entries = ref<FileEntry[]>([]);
    const selectedPaths = ref<string[]>([]);
    const containerRef = ref<HTMLElement | null>(null);

    let submittedBatch: any = null;
    transferStore.submitUploadBatch = (async (options: any) => {
      submittedBatch = options;
      return { successfulCount: 1, failedCount: 0, cancelled: false };
    }) as any;

    const dnd = useFileDragDrop({
      panelId: 'left',
      connectionId,
      currentPath,
      entries,
      selectedPaths,
      containerRef,
      onSelect: () => {},
    });

    const targetFolder: FileEntry = {
      name: 'reports',
      path: '/documents/reports',
      kind: 'directory',
      size: 0,
      modified: '2026-01-01T00:00:00Z',
      is_hidden: false,
      is_symlink: false,
    };

    const file = new File(['pdf data'], 'annual.pdf', { type: 'application/pdf' });
    const mockEvent = {
      preventDefault: () => {},
      stopPropagation: () => {},
      dataTransfer: {
        getData: () => '',
        items: [],
        files: [file],
      },
    } as unknown as DragEvent;

    await dnd.handleDrop(mockEvent, targetFolder);

    expect(submittedBatch).not.toBeNull();
    expect(submittedBatch.targetDir).toBe('/documents/reports');
    expect(submittedBatch.files[0].targetDir).toBe('/documents/reports');
  });

  it('prevents internal same-directory move without triggering transfer', async () => {
    const transferStore = useTransferStore();
    const currentPath = ref('/documents');
    const connectionId = ref('local');
    const entries = ref<FileEntry[]>([]);
    const selectedPaths = ref<string[]>([]);
    const containerRef = ref<HTMLElement | null>(null);

    let transferSubmitted = false;
    transferStore.submitTransfer = (async () => {
      transferSubmitted = true;
    }) as any;

    const dnd = useFileDragDrop({
      panelId: 'left',
      connectionId,
      currentPath,
      entries,
      selectedPaths,
      containerRef,
      onSelect: () => {},
    });

    const payload = JSON.stringify({
      sourcePanelId: 'left',
      sourceConnectionId: 'local',
      paths: ['/documents/readme.md'],
    });

    const mockEvent = {
      preventDefault: () => {},
      stopPropagation: () => {},
      shiftKey: false,
      ctrlKey: false,
      altKey: false,
      dataTransfer: {
        getData: (format: string) => (format === 'application/json' ? payload : ''),
      },
    } as unknown as DragEvent;

    await dnd.handleDrop(mockEvent);

    expect(transferSubmitted).toBe(false);
  });

  it('handleDragEnter activates isDragOver for external or cross-panel drags', () => {
    const currentPath = ref('/documents');
    const connectionId = ref('local');
    const entries = ref<FileEntry[]>([]);
    const selectedPaths = ref<string[]>([]);
    const containerRef = ref<HTMLElement | null>(null);

    const dnd = useFileDragDrop({
      panelId: 'left',
      connectionId,
      currentPath,
      entries,
      selectedPaths,
      containerRef,
      onSelect: () => {},
    });

    const mockEvent = {
      preventDefault: () => {},
      shiftKey: false,
    } as unknown as DragEvent;

    expect(dnd.isDragOver.value).toBe(false);
    dnd.handleDragEnter(mockEvent);
    expect(dnd.isDragOver.value).toBe(true);
  });

  it('handleDragEnter suppresses generic dropzone overlay when drag originated from self panel', () => {
    const currentPath = ref('/documents');
    const connectionId = ref('local');
    const entry: FileEntry = {
      name: 'file.txt',
      path: '/documents/file.txt',
      kind: 'file',
      size: 100,
      modified: 0,
      is_hidden: false,
    };
    const entries = ref<FileEntry[]>([entry]);
    const selectedPaths = ref<string[]>(['/documents/file.txt']);
    const containerRef = ref<HTMLElement | null>(null);

    const dnd = useFileDragDrop({
      panelId: 'left',
      connectionId,
      currentPath,
      entries,
      selectedPaths,
      containerRef,
      onSelect: () => {},
    });

    // Start drag from this panel
    dnd.handleDragStart({
      dataTransfer: {
        setData: () => {},
      },
    } as unknown as DragEvent, entry);

    expect(dnd.draggedPaths.value).toContain('/documents/file.txt');

    // Drag enter should now be suppressed for self panel
    const mockEnterEvent = {
      preventDefault: () => {},
      shiftKey: false,
    } as unknown as DragEvent;

    dnd.handleDragEnter(mockEnterEvent);
    expect(dnd.isDragOver.value).toBe(false);
  });
});
