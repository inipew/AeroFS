import type { PanelId } from '../types/workspace';
import type { Connection } from '../types/connection';

/**
 * Single source of truth for all modal / dialog / popover overlays in AeroFS.
 * Invariant: Stores minimal intent/identifier only, not heavy domain payloads.
 */
export type Overlay =
  | { type: 'create'; initialType?: 'file' | 'directory'; panelId: PanelId }
  | { type: 'rename'; panelId: PanelId; path: string }
  | { type: 'delete'; panelId: PanelId; paths: string[] }
  | { type: 'upload'; panelId: PanelId }
  | { type: 'connection'; connectionToEdit?: Connection | null }
  | { type: 'delete-connection'; connection: Connection }
  | { type: 'archive'; connectionId: string; basePath: string; selectedPaths: string[] }
  | { type: 'archive-viewer'; connectionId: string; archivePath: string }
  | { type: 'search' }
  | { type: 'settings' }
  | { type: 'shares' }
  | { type: 'trash' }
  | { type: 'starred' }
  | { type: 'recent' }
  | { type: 'properties'; connectionId: string; path: string }
  | { type: 'create-share'; connectionId: string; path: string }
  | { type: 'sync'; sourceConnection?: string; sourcePath?: string; destConnection?: string; destPath?: string }
  | { type: 'editor'; connectionId: string; path: string }
  | { type: 'media-viewer'; connectionId: string; path: string }
  | { type: 'command-palette' };
