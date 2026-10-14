/**
 * Type definitions for Engram VS Code Extension
 */

export interface DoctorItem {
  memory_id: string;
  title: string;
  scope_level: string;
  scope_target: string | null;
  liveness: string;
  diagnostic_message: string;
  suggested_actions: string[];
}

export interface DoctorReport {
  protocol_version: string;
  total_memories: number;
  live_count: number;
  dormant_count: number;
  stale_count: number;
  superseded_count: number;
  archival_candidate_count: number;
  items: DoctorItem[];
  event_chain_status: string;
  event_chain_details: string;
}

export type BinaryResolutionSource =
  | 'config'
  | 'env_engram_path'
  | 'env_install_dir'
  | 'path'
  | 'standard_location'
  | 'developer_fallback';

export interface BinaryResolutionSuccess {
  readonly valid: true;
  readonly path: string;
  readonly version: string;
  readonly source: BinaryResolutionSource;
}

export interface BinaryResolutionFailure {
  readonly valid: false;
  readonly attemptedPath?: string;
  readonly error: string;
  readonly source?: BinaryResolutionSource;
  readonly isExplicitConfigError: boolean;
}

export type BinaryResolutionResult = BinaryResolutionSuccess | BinaryResolutionFailure;

export interface EngramConfig {
  binaryPath: string;
  timeoutMs: number;
}
