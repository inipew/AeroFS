import { ref, onMounted, onUnmounted, getCurrentInstance, type Ref } from 'vue';
import type { FileEntry } from '../../../types/vfs';
import type { PanelId } from '../../../types/workspace';
import { useTransferStore } from '../../../stores/transferStore';
import { useUiStore } from '../../../stores/uiStore';
import { listFilesApi } from '../../../api/files';
import { generateConflictResolvedName } from '../../../utils/naming';

export interface UseFileDragDropOptions {
  panelId: PanelId;
  connectionId: Ref<string>;
  currentPath: Ref<string>;
  entries: Ref<FileEntry[]>;
  selectedPaths: Ref<string[]>;
  containerRef: Ref<HTMLElement | null>;
  onSelect: (paths: string[]) => void;
}

export interface DroppedUploadItem {
  file: File;
  relativePath: string;
}

export function useFileDragDrop(options: UseFileDragDropOptions) {
  const transferStore = useTransferStore();
  const uiStore = useUiStore();

  const isDragOver = ref(false);
  const hoveredFolderDrop = ref<string | null>(null);
  const draggedPaths = ref<string[]>([]);
  const isShiftPressed = ref(false);
  let dragEnterCounter = 0;

  function handleDragEnter(e: DragEvent) {
    e.preventDefault();
    dragEnterCounter++;
    isDragOver.value = true;
    isShiftPressed.value = e.shiftKey;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    const currentTarget = e.currentTarget as HTMLElement | null;
    const relatedTarget = e.relatedTarget as HTMLElement | null;
    if (currentTarget && relatedTarget && currentTarget.contains(relatedTarget)) {
      return;
    }
    dragEnterCounter--;
    if (
      dragEnterCounter <= 0 ||
      !relatedTarget ||
      (options.containerRef.value && !options.containerRef.value.contains(relatedTarget))
    ) {
      dragEnterCounter = 0;
      isDragOver.value = false;
      hoveredFolderDrop.value = null;
    }
  }

  function handleFolderDragOver(e: DragEvent, folder: FileEntry) {
    e.preventDefault();
    hoveredFolderDrop.value = folder.path;
    isShiftPressed.value = e.shiftKey;
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = e.shiftKey ? 'move' : 'copy';
    }
  }

  function handleFolderDragLeave(e: DragEvent, folder: FileEntry) {
    const currentTarget = e.currentTarget as HTMLElement | null;
    const relatedTarget = e.relatedTarget as HTMLElement | null;
    if (currentTarget && relatedTarget && currentTarget.contains(relatedTarget)) {
      return;
    }
    if (hoveredFolderDrop.value === folder.path) {
      hoveredFolderDrop.value = null;
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isShiftPressed.value = e.shiftKey;
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = e.shiftKey ? 'move' : 'copy';
    }
  }

  function handleDragStart(e: DragEvent, entry: FileEntry) {
    const selected = options.selectedPaths.value.includes(entry.path)
      ? options.selectedPaths.value
      : [entry.path];

    draggedPaths.value = selected;

    if (!options.selectedPaths.value.includes(entry.path)) {
      options.onSelect([entry.path]);
    }

    const payload = {
      sourcePanelId: options.panelId,
      sourceConnectionId: options.connectionId.value,
      paths: selected,
    };

    const payloadStr = JSON.stringify(payload);
    e.dataTransfer?.setData('application/json', payloadStr);
    e.dataTransfer?.setData('text/plain', payloadStr);
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'copyMove';

      if (selected.length > 1) {
        const badge = document.createElement('div');
        badge.style.position = 'absolute';
        badge.style.top = '-9999px';
        badge.style.left = '-9999px';
        badge.style.padding = '6px 14px';
        badge.style.background = '#2563eb';
        badge.style.color = '#ffffff';
        badge.style.fontWeight = '600';
        badge.style.fontSize = '12px';
        badge.style.borderRadius = '9999px';
        badge.style.boxShadow = '0 10px 15px -3px rgba(0,0,0,0.3)';
        badge.textContent = `${selected.length} items`;
        document.body.appendChild(badge);
        e.dataTransfer.setDragImage(badge, 20, 20);
        setTimeout(() => {
          if (badge.parentNode) {
            badge.parentNode.removeChild(badge);
          }
        }, 0);
      }
    }
  }

  async function traverseDirectoryEntry(entry: any, currentPath: string): Promise<DroppedUploadItem[]> {
    const items: DroppedUploadItem[] = [];
    if (entry.isFile) {
      const file: File = await new Promise((resolve, reject) => entry.file(resolve, reject));
      items.push({
        file,
        relativePath: currentPath ? `${currentPath}/${entry.name}` : entry.name,
      });
    } else if (entry.isDirectory) {
      const dirReader = entry.createReader();
      const readAllEntries = async (): Promise<any[]> => {
        const entries: any[] = [];
        let batch: any[] = [];
        do {
          batch = await new Promise((resolve, reject) => dirReader.readEntries(resolve, reject));
          entries.push(...batch);
        } while (batch.length > 0);
        return entries;
      };

      const dirEntries = await readAllEntries();
      const nextPath = currentPath ? `${currentPath}/${entry.name}` : entry.name;
      for (const child of dirEntries) {
        const childItems = await traverseDirectoryEntry(child, nextPath);
        items.push(...childItems);
      }
    }
    return items;
  }

  async function handleExternalFilesDrop(e: DragEvent, targetDir: string) {
    const items = e.dataTransfer?.items;
    const files = e.dataTransfer?.files;
    if (!items && !files) return;

    const uploadItems: DroppedUploadItem[] = [];

    if (items && items.length > 0) {
      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        let entry: any = null;
        try {
          if (item.webkitGetAsEntry) {
            entry = item.webkitGetAsEntry();
          }
        } catch {
          entry = null;
        }

        if (entry) {
          const entryItems = await traverseDirectoryEntry(entry, '');
          uploadItems.push(...entryItems);
        } else if (item.kind === 'file') {
          const f = item.getAsFile();
          if (f) {
            uploadItems.push({ file: f, relativePath: f.name });
          }
        }
      }
    }

    // Fallback if webkitGetAsEntry failed to collect items or items was empty, but files exist
    if (uploadItems.length === 0 && files && files.length > 0) {
      for (let i = 0; i < files.length; i++) {
        const f = files[i];
        uploadItems.push({ file: f, relativePath: f.name });
      }
    }

    if (uploadItems.length === 0) return;

    const connId = options.connectionId.value || 'local';
    const existingFileNames = options.entries.value.map((ent) => ent.name);

    const batchItems = uploadItems.map((item) => {
      const cleanRel = item.relativePath.replace(/^\/+/, '');
      const lastSlash = cleanRel.lastIndexOf('/');
      const subDir = lastSlash > -1 ? cleanRel.substring(0, lastSlash) : '';
      const destDir = subDir
        ? targetDir === '/'
          ? `/${subDir}`
          : `${targetDir}/${subDir}`
        : targetDir;
      return {
        file: item.file,
        targetDir: destDir,
      };
    });

    void transferStore.submitUploadBatch({
      connectionId: connId,
      targetDir,
      files: batchItems,
      existingNames: existingFileNames,
    });
  }

  async function handleDrop(e: DragEvent, targetFolder?: FileEntry) {
    e.preventDefault();
    e.stopPropagation();
    dragEnterCounter = 0;
    isDragOver.value = false;
    hoveredFolderDrop.value = null;
    draggedPaths.value = [];
    isShiftPressed.value = false;

    const targetDir =
      targetFolder && targetFolder.kind === 'directory'
        ? targetFolder.path
        : options.currentPath.value;

    const rawData =
      e.dataTransfer?.getData('application/json') || e.dataTransfer?.getData('text/plain');
    if (!rawData) {
      await handleExternalFilesDrop(e, targetDir);
      return;
    }

    try {
      const data = JSON.parse(rawData);
      if (!data.paths || data.paths.length === 0) return;

      const isMove =
        e.shiftKey ||
        (data.sourceConnectionId === options.connectionId.value && !e.ctrlKey && !e.altKey);
      const opType: 'copy' | 'move' = isMove ? 'move' : 'copy';
      const opLabel = isMove ? 'Move' : 'Copy';

      // Prevent moving into exact same directory on same connection
      if (
        opType === 'move' &&
        data.sourceConnectionId === options.connectionId.value &&
        data.paths.every((p: string) => {
          const parent = p.substring(0, p.lastIndexOf('/')) || '/';
          return parent === targetDir;
        })
      ) {
        return;
      }

      // Prevent dropping a folder into itself or descendant
      for (const filePath of data.paths) {
        if (data.sourceConnectionId === options.connectionId.value) {
          if (targetDir === filePath || targetDir.startsWith(filePath + '/')) {
            uiStore.showToast('Cannot copy or move a folder into itself or its subfolder', 'error');
            return;
          }
        }
      }

      let targetEntries: FileEntry[] = [];
      try {
        const resp = await listFilesApi(options.connectionId.value, { path: targetDir });
        targetEntries = (resp.entries || []) as FileEntry[];
      } catch {
        targetEntries = options.entries.value;
      }

      for (const filePath of data.paths) {
        let fileName = filePath.split('/').pop() || 'file';
        let targetPath = targetDir === '/' ? `/${fileName}` : `${targetDir}/${fileName}`;

        const alreadyExists = targetEntries.some((ent) => ent.name === fileName);
        if (alreadyExists) {
          const resolution = await transferStore.requestConflict(fileName, filePath, targetPath);
          if (resolution === 'cancel') break;
          if (resolution === 'skip') continue;
          if (resolution === 'keep_both') {
            fileName = generateConflictResolvedName(fileName, targetEntries.map((e) => e.name));
            targetPath = targetDir === '/' ? `/${fileName}` : `${targetDir}/${fileName}`;
          }
        }

        await transferStore.submitTransfer(
          `${opLabel} ${fileName} to ${targetDir}`,
          opType,
          data.sourceConnectionId,
          filePath,
          options.connectionId.value,
          targetPath
        );
      }
    } catch {
      await handleExternalFilesDrop(e, targetDir);
    }
  }

  const onGlobalDragEnd = () => {
    dragEnterCounter = 0;
    isDragOver.value = false;
    hoveredFolderDrop.value = null;
    draggedPaths.value = [];
    isShiftPressed.value = false;
  };

  if (getCurrentInstance()) {
    onMounted(() => {
      window.addEventListener('dragend', onGlobalDragEnd);
      window.addEventListener('drop', onGlobalDragEnd);
    });

    onUnmounted(() => {
      window.removeEventListener('dragend', onGlobalDragEnd);
      window.removeEventListener('drop', onGlobalDragEnd);
    });
  }

  return {
    isDragOver,
    hoveredFolderDrop,
    draggedPaths,
    isShiftPressed,
    handleDragEnter,
    handleDragLeave,
    handleFolderDragOver,
    handleFolderDragLeave,
    handleDragOver,
    handleDragStart,
    handleDrop,
  };
}
