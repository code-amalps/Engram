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

export interface MemoryScope {
  level: string;
  target: string | null;
  team: string | null;
  modules: string[];
  files: string[];
  symbols: string[];
}

export interface MemorySource {
  source_type: string;
  reference: string | null;
}

export interface DurableMemory {
  id: string;
  title: string;
  statement: string;
  reason: string;
  accepted_by: string;
  accepted_at: string;
  scope: MemoryScope;
  evidence_tier?: string;
  lifecycle_state: string;
  supersedes: string | null;
  superseded_by: string | null;
  tags: string[];
  ratified_at_commit?: string | null;
  ratified_by?: string | null;
  ratified_at?: string | null;
}

export interface MemoryListResponse {
  version: number;
  memories: DurableMemory[];
}

export interface MemoryCandidate {
  candidate_id: string;
  task_id: string;
  title: string;
  statement: string;
  provisional_reason: string;
  proposed_by: string;
  scope: MemoryScope;
  created_at: string;
  lifecycle_state: string;
}

export interface ContextFile {
  path: string;
  score: number;
  level: number;
  provenance: string[];
  content: string;
}

export interface ContextReport {
  task: string;
  project: string;
  revision: string;
  indexed_files: number;
  token_budget: number;
  estimated_tokens: number;
  confidence: string;
  constraints: string[];
  relevant_memories: Array<{ id: string; title: string; statement?: string }>;
  cochange_alerts: string[];
  ecosystem_guard: string;
  gaps: string[];
  files: ContextFile[];
  excluded_files: Array<{ path: string; reason: string }>;
}

export interface RecordDecisionOptions {
  title: string;
  statement: string;
  reason: string;
  modules?: string[];
  files?: string[];
  acceptedBy?: string;
}
