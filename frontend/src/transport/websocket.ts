import type { TransferJob } from '../types/transfer';
import { publishFileChange } from '../services/fileChangeBus';

export interface FileChangeEvent {
  connection_id: string;
  path: string;
  action: string;
}

export interface ResyncRequiredEvent {
  reason: string;
  latest_sequence: number;
}

export type RealtimeListener<T> = (data: T) => void;

export class RealtimeClient {
  private socket: WebSocket | null = null;
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

  public connect(): void {
    if (this.socket && (this.socket.readyState === WebSocket.OPEN || this.socket.readyState === WebSocket.CONNECTING)) {
      return;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const query = this.lastSequence > 0 ? `?last_seq=${this.lastSequence}` : '';
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

          // Sequence Gap Detection & Auto-Resync (Plan 40 P0.2, P1.13)
          if (typeof payload.sequence === 'number') {
            if (this.lastSequence > 0 && payload.sequence > this.lastSequence + 1) {
              console.warn(
                `[RealtimeClient] Sequence gap detected (${this.lastSequence} -> ${payload.sequence}). Requesting resync.`
              );
              this.resyncListeners.forEach((l) =>
                l({ reason: 'sequence_gap', latest_sequence: payload.sequence })
              );
            }
            this.lastSequence = Math.max(this.lastSequence, payload.sequence);
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
              });
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

  public onStatusChange(listener: RealtimeListener<boolean>): () => void {
    this.statusListeners.add(listener);
    return () => this.statusListeners.delete(listener);
  }
}

export const realtimeClient = new RealtimeClient();
