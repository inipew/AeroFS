import type { FileEntry } from '../types/vfs';
import type { PanelStatus } from '../types/workspace';

/**
 * PanelSession — explicit lifecycle unit (66.md §2, §5).
 * Each panel owns its generation, abort controller, and closed flag.
 * Stale responses are discarded via generation mismatch, not global counters.
 */
export class PanelSession {
  generation = 0;
  abortController: AbortController | null = null;
  closed = false;
  entries: FileEntry[] = [];
  status: PanelStatus = 'idle';
  error: string | null = null;
  lastLoadedAt?: number;
  lastError?: string;
  readonly id: 'left' | 'right';
  connectionId: string;
  path: string;

  constructor(id: 'left' | 'right', connectionId: string, path: string) {
    this.id = id;
    this.connectionId = connectionId;
    this.path = path;
  }

  /** Increment generation and abort in-flight request — call on navigate/refresh/close. */
  bumpGeneration(): number {
    this.generation++;
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
    return this.generation;
  }

  newRequest(): { generation: number; signal: AbortSignal } {
    const generation = this.bumpGeneration();
    this.abortController = new AbortController();
    return { generation, signal: this.abortController.signal };
  }

  isStale(generation: number): boolean {
    return this.closed || generation !== this.generation;
  }

  dispose(): void {
    this.closed = true;
    this.bumpGeneration();
    this.entries = [];
    this.error = null;
  }

  markClosed(): void {
    this.closed = true;
    this.status = 'closed';
    this.bumpGeneration();
  }
}
