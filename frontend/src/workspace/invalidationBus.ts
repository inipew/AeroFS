import { normalizePath } from '../utils/path';

/**
 * InvalidationBus — targeted directory invalidation (66.md §18-20).
 * Only affected panels (connectionId + normalizedPath) reload.
 */
export type InvalidationHandler = (connectionId: string, dirPath: string) => void;

export class InvalidationBus {
  private handlers = new Set<InvalidationHandler>();
  private pending = new Set<string>(); // "conn::path"
  private timer: ReturnType<typeof setTimeout> | null = null;

  subscribe(handler: InvalidationHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  queue(connectionId: string, dirPath: string): void {
    this.pending.add(`${connectionId}::${normalizePath(dirPath)}`);
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.flush();
    }, 150);
  }

  flush(): void {
    if (this.timer) { clearTimeout(this.timer); this.timer = null; }
    for (const item of this.pending) {
      const [connId, dPath] = item.split('::');
      for (const h of this.handlers) h(connId, dPath);
    }
    this.pending.clear();
  }

  cancel(): void {
    if (this.timer) { clearTimeout(this.timer); this.timer = null; }
    this.pending.clear();
  }
}

export const invalidationBus = new InvalidationBus();
