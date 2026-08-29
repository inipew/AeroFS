export type ProviderKind = 'local' | 'ftp' | 'ftps' | 'sftp';

export type ProviderState =
  | 'initializing'
  | 'connecting'
  | 'ready'
  | 'degraded'
  | 'draining'
  | 'disconnected'
  | 'failed';

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'failed';

export interface Connection {
  id: string;
  name: string;
  provider: ProviderKind;
  provider_type?: string;
  provider_state?: ProviderState;
  host?: string;
  port?: number;
  username?: string;
  base_path: string;
  read_only: boolean;
  enabled: boolean;
  status: ConnectionStatus;
  created_at: string;
  updated_at: string;
}
