export type TransferType = 'copy' | 'move' | 'upload' | 'sync';

export type TransferStatus =
  | 'queued'
  | 'running'
  | 'cancellation_requested'
  | 'cancelled'
  | 'interrupted'
  | 'completed'
  | 'failed';

export type TransferPhase =
  | 'preparing'
  | 'transferring'
  | 'finalizing'
  | 'verifying'
  | 'cleaning_up'
  | 'completed';

export type TransferExecutionMode = 'inline' | 'background' | 'resumable';
export type TransferStaging = 'none' | 'local_temp' | 'provider_temp';

export interface TransferJob {
  id: string;
  user_id?: string;
  name: string;
  transfer_type: TransferType;
  source_connection_id: string;
  source_path: string;
  destination_connection_id: string;
  destination_path: string;
  status: TransferStatus;
  phase?: TransferPhase;
  execution_mode: TransferExecutionMode;
  staging: TransferStaging;
  transferred_bytes: number;
  total_bytes: number;
  speed_bytes_per_sec: number;
  eta_seconds?: number;
  checksum?: string;
  error_message?: string;
  dismissed_at?: string;
  created_at: string;
  updated_at: string;
}
