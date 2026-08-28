export interface FileChangeEvent {
  connectionId: string;
  path: string;
  action: 'create' | 'write' | 'delete' | 'rename' | string;
}

type Listener = (event: FileChangeEvent) => void;

const listeners = new Set<Listener>();

export function subscribeFileChanges(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function publishFileChange(event: FileChangeEvent): void {
  for (const listener of listeners) {
    try {
      listener(event);
    } catch (e) {
      console.error('[FileChangeBus] Listener error', e);
    }
  }
}
