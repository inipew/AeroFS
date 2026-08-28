import type { FileEntry } from './vfs';

export type PanelId = 'left' | 'right';
export type WorkspaceLayout = 'single' | 'split';
export type PanelStatus = 'idle' | 'loading' | 'refreshing' | 'stale' | 'error' | 'offline';
export type SortField = 'name' | 'size' | 'modified' | 'type';
export type SortOrder = 'asc' | 'desc';
export type ViewMode = 'grid' | 'list';

export interface PanelLocation {
  connectionId: string;
  path: string;
}

export interface NavigationState {
  history: string[];
  historyIndex: number;
}

export interface PanelViewState {
  viewMode: ViewMode;
  showHidden: boolean;
  sortField: SortField;
  sortOrder: SortOrder;
  filterType: string;
  searchQuery: string;
}

export interface SelectionState {
  paths: string[];
  focusedPath?: string;
}

export interface PanelRuntimeState {
  entries: FileEntry[];
  status: PanelStatus;
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  initialized: boolean;
}

export interface Panel {
  id: PanelId;
  location: PanelLocation;
  navigation: NavigationState;
  view: PanelViewState;
  selection: SelectionState;
  runtime: PanelRuntimeState;

  // Flattened ergonomic / backward-compatible properties
  connectionId: string;
  path: string;
  entries: FileEntry[];
  selectedEntries: string[];
  viewMode: ViewMode;
  showHidden: boolean;
  sortField: string;
  sortOrder: SortOrder;
  filterType: string;
  searchQuery: string;
  loading: boolean;
  error: string | null;
  stale?: boolean;
  history: string[];
  historyIndex: number;
  initialized: boolean;
}

export type PanelState = Panel;

export interface WorkspaceClipboard {
  operation: 'copy' | 'cut';
  sourceConnectionId: string;
  sourcePanelId: PanelId;
  paths: string[];
  items?: FileEntry[];
}

export interface PersistedPanelLocation {
  connectionId: string;
  path: string;
  viewMode: ViewMode;
  showHidden: boolean;
  sortField: SortField;
  sortOrder: SortOrder;
}

export interface PersistedWorkspace {
  version: 1;
  layout: WorkspaceLayout;
  activePanel: PanelId;
  splitRatio?: number;
  left: PersistedPanelLocation;
  right: PersistedPanelLocation;
}
