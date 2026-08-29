import type { TransferJob } from '../types/transfer';
import { publishFileChange } from '../services/fileChangeBus';

export interface FileChangeEvent {
  connection_id: string;
  path: string;
  action: string;
  old_path?: string;
  parent_path?: string;
  old_parent_path?: string;
}

export interface ResyncRequiredEvent {
  reason: string;
  latest_sequence?: number;
  epoch?: string;
}

export type RealtimeListener<T> = (data: T) => void;

export class RealtimeClient {
  private socket: WebSocket | null = null;
  private lastEpoch: string | null = null;
  private lastSequence: number = 0;
  private isConnected: boolean = false;
  private retryAttempt: number = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  private progressListeners: Set<RealtimeListener<TransferJob>> = new Set();
  private completedListeners: Set<RealtimeListener<TransferJob>> = new Set();
  private failedListeners: Set<RealtimeListener<TransferJob>> = new Set();
  private fileChangeListeners: Set<RealtimeListener<FileChangeEvent>> = new Set();
  private resyncListeners: Set<RealtimeListener<ResyncRequiredEvent>> = new Set();
  private statusListeners: Set<RealtimeListener<boolean>> = new Set();
  private permissionListeners: Set<RealtimeListener<{ user_id: string; connection_id: string }>> = new Set();

  private visibilityHandler: (() => void) | null = null;

  constructor() {
    if (typeof window !== 'undefined' && window.sessionStorage) {
      try {
        this.lastEpoch = window.sessionStorage.getItem('aerofs_ws_epoch');
        const seq = window.sessionStorage.getItem('aerofs_ws_seq');
        if (seq) {
          this.lastSequence = parseInt(seq, 10) || 0;
        }
      } catch (_) {}
    }

    if (typeof document !== 'undefined') {
      this.visibilityHandler = () => {
        if (document.visibilityState === 'visible') {
          if (!this.isConnected) {
            this.connect();
          } else {
            // Trigger resync to catch up on any missed background events
            this.resyncListeners.forEach((l) =>
              l({ reason: 'visibility_resumed', latest_sequence: this.lastSequence, epoch: this.lastEpoch || undefined })
            );
          }
        }
      };
      document.addEventListener('visibilitychange', this.visibilityHandler);
    }
  }

  private persistCursor(epoch: string | null, seq: number): void {
    this.lastEpoch = epoch;
    this.lastSequence = seq;
    if (typeof window !== 'undefined' && window.sessionStorage) {
      try {
        if (epoch) window.sessionStorage.setItem('aerofs_ws_epoch', epoch);
        window.sessionStorage.setItem('aerofs_ws_seq', String(seq));
      } catch (_) {}
    }
  }

  public start(): void {
    this.connect();
  }

  public stop(): void {
    this.disconnect();
  }

  public dispose(): void {
    this.disconnect();
    if (typeof document !== 'undefined' && this.visibilityHandler) {
      document.removeEventListener('visibilitychange', this.visibilityHandler);
      this.visibilityHandler = null;
    }
    this.progressListeners.clear();
    this.completedListeners.clear();
    this.failedListeners.clear();
    this.fileChangeListeners.clear();
    this.resyncListeners.clear();
    this.statusListeners.clear();
    this.permissionListeners.clear();
  }

  public connect(): void {
    if (this.socket && (this.socket.readyState === WebSocket.OPEN || this.socket.readyState === WebSocket.CONNECTING)) {
      return;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const params = new URLSearchParams();
    if (this.lastSequence > 0) {
      params.set('last_seq', String(this.lastSequence));
    }
    if (this.lastEpoch) {
      params.set('last_epoch', this.lastEpoch);
    }
    const query = params.toString() ? `?${params.toString()}` : '';
    const url = `${protocol}//${window.location.host}/api/v1/ws${query}`;

    try {
      this.socket = new WebSocket(url);

      this.socket.onopen = () => {
        this.isConnected = true;
        this.retryAttempt = 0;
        if (this.reconnectTimer) {
          clearTimeout(this.reconnectTimer);
          this.reconnectTimer = null;
        }
        this.statusListeners.forEach((l) => l(true));
      };

      this.socket.onmessage = (event) => {
        try {
          const payload = JSON.parse(event.data);

          // Handle Epoch Announcement & Full Sync
          if (payload.type === 'epoch_info' && payload.data) {
            this.persistCursor(payload.data.epoch, payload.data.latest_sequence || this.lastSequence);
            return;
          }

          if (payload.type === 'full_sync' && payload.data) {
            console.warn('[RealtimeClient] Server epoch changed (restart detected). Triggering full store sync.');
            this.persistCursor(payload.data.epoch, payload.data.latest_sequence || 0);
            this.resyncListeners.forEach((l) =>
              l({ reason: 'epoch_changed', latest_sequence: payload.data.latest_sequence, epoch: payload.data.epoch })
            );
            return;
          }

          // Sequence Gap & Out-of-Order Stale Message Protection (Plan 54 P1.18)
          if (typeof payload.sequence === 'number') {
            if (this.lastSequence > 0 && payload.sequence <= this.lastSequence) {
              console.warn(
                `[RealtimeClient] Stale out-of-order message received (seq ${payload.sequence} <= ${this.lastSequence}). Discarding.`
              );
              return;
            }
            if (this.lastSequence > 0 && payload.sequence > this.lastSequence + 1) {
              console.warn(
                `[RealtimeClient] Sequence gap detected (${this.lastSequence} -> ${payload.sequence}). Requesting resync.`
              );
              this.resyncListeners.forEach((l) =>
                l({ reason: 'sequence_gap', latest_sequence: payload.sequence, epoch: this.lastEpoch || undefined })
              );
            }
            this.persistCursor(this.lastEpoch, payload.sequence);
          }

          switch (payload.type) {
            case 'transfer_progress':
              this.progressListeners.forEach((l) => l(payload.data));
              break;
            case 'transfer_completed':
              this.completedListeners.forEach((l) => l(payload.data));
              break;
            case 'transfer_failed':
              this.failedListeners.forEach((l) => l(payload.data));
              break;
            case 'file_change':
              this.fileChangeListeners.forEach((l) => l(payload.data));
              publishFileChange({
                connectionId: payload.data.connection_id,
                path: payload.data.path,
                action: payload.data.action,
                oldPath: payload.data.old_path,
                parentPath: payload.data.parent_path,
                oldParentPath: payload.data.old_parent_path,
              });
              break;
            case 'permission_changed':
              this.permissionListeners.forEach((l) => l(payload.data));
              break;
            case 'resync_required':
              this.resyncListeners.forEach((l) => l(payload.data));
              break;
          }
        } catch (e) {
          console.error('[RealtimeClient] Failed to parse WebSocket message', e);
        }
      };

      this.socket.onclose = () => {
        this.handleDisconnect();
      };

      this.socket.onerror = () => {
        this.socket?.close();
      };
    } catch (err) {
      console.error('[RealtimeClient] Connection error', err);
      this.handleDisconnect();
    }
  }

  public disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.socket) {
      this.socket.onclose = null;
      this.socket.onerror = null;
      this.socket.close();
      this.socket = null;
    }
    this.isConnected = false;
    this.statusListeners.forEach((l) => l(false));
  }

  private handleDisconnect(): void {
    if (this.isConnected) {
      this.isConnected = false;
      this.statusListeners.forEach((l) => l(false));
    }
    this.socket = null;

    if (!this.reconnectTimer) {
      // Exponential backoff: 1s, 2s, 4s, 8s, up to max 30s
      const delay = Math.min(1000 * Math.pow(2, this.retryAttempt), 30000);
      this.retryAttempt++;
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this.connect();
      }, delay);
    }
  }

  public getConnected(): boolean {
    return this.isConnected;
  }

  public getLastEpoch(): string | null {
    return this.lastEpoch;
  }

  public getLastSequence(): number {
    return this.lastSequence;
  }

  public onProgress(listener: RealtimeListener<TransferJob>): () => void {
    this.progressListeners.add(listener);
    return () => this.progressListeners.delete(listener);
  }

  public onCompleted(listener: RealtimeListener<TransferJob>): () => void {
    this.completedListeners.add(listener);
    return () => this.completedListeners.delete(listener);
  }

  public onFailed(listener: RealtimeListener<TransferJob>): () => void {
    this.failedListeners.add(listener);
    return () => this.failedListeners.delete(listener);
  }

  public onFileChange(listener: RealtimeListener<FileChangeEvent>): () => void {
    this.fileChangeListeners.add(listener);
    return () => this.fileChangeListeners.delete(listener);
  }

  public onResyncRequired(listener: RealtimeListener<ResyncRequiredEvent>): () => void {
    this.resyncListeners.add(listener);
    return () => this.resyncListeners.delete(listener);
  }

  public onPermissionChanged(listener: RealtimeListener<{ user_id: string; connection_id: string }>): () => void {
    this.permissionListeners.add(listener);
    return () => this.permissionListeners.delete(listener);
  }

  public onStatusChange(listener: RealtimeListener<boolean>): () => void {
    this.statusListeners.add(listener);
    return () => this.statusListeners.delete(listener);
  }
}

export const realtimeClient = new RealtimeClient();
