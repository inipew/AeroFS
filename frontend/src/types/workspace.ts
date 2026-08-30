import type { FileEntry } from './vfs';

export type PanelId = 'left' | 'right';
export type WorkspaceLayout = 'single' | 'split';
export type PanelStatus = 'idle' | 'loading' | 'refreshing' | 'loading_more' | 'stale' | 'error' | 'offline' | 'degraded' | 'orphaned' | 'closed';
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
  direction?: 'forward' | 'back' | 'replace';
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
  loadingMore: boolean;
  error: string | null;
  initialized: boolean;
  hasMore: boolean;
  nextCursor?: string;
  totalCount?: number;
  lastLoadedAt?: number;
  lastError?: string;
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
  loadingMore?: boolean;
  error: string | null;
  stale?: boolean;
  history: string[];
  historyIndex: number;
  navigationDirection?: 'forward' | 'back' | 'replace';
  initialized: boolean;
  hasMore?: boolean;
  nextCursor?: string;
  totalCount?: number;
  lastLoadedAt?: number;
  lastError?: string;
  status: PanelStatus;
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
  history?: string[];
  historyIndex?: number;
}

export interface PersistedWorkspace {
  version: 1;
  layout: WorkspaceLayout;
  activePanel: PanelId;
  splitRatio?: number;
  left: PersistedPanelLocation;
  right: PersistedPanelLocation;
}
