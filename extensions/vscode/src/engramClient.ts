import * as cp from 'child_process';
import * as vscode from 'vscode';
import {
  DoctorReport,
  DoctorItem,
  DurableMemory,
  MemoryCandidate,
  ContextReport,
  RecordDecisionOptions,
} from './types';
import { EngramOutputChannel } from './outputChannel';

export interface CliExecutionResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  durationMs: number;
}

export class EngramClient {
  private isRunningHealthCheck = false;
  private isRunningCompileContext = false;

  constructor(private outputChannel: EngramOutputChannel) {}

  /**
   * Executes a CLI command and parses the returned JSON.
   */
  public async executeJson<T>(
    executablePath: string,
    args: string[],
    cwd: string,
    timeoutMs: number,
    cancellationToken?: vscode.CancellationToken
  ): Promise<T> {
    const startTime = Date.now();
    const result = await this.executeProcess(executablePath, args, cwd, timeoutMs, cancellationToken);
    const durationMs = Date.now() - startTime;

    if (result.exitCode !== 0) {
      const errorMsg = result.stderr.trim() || result.stdout.trim() || `Process exited with code ${result.exitCode}`;
      this.outputChannel.log(`CLI execution error (${durationMs}ms): ${errorMsg}`, 'ERROR');
      throw new Error(`Engram CLI failed: ${errorMsg}`);
    }

    if (!result.stdout || result.stdout.trim() === '') {
      throw new Error('Engram CLI returned empty output.');
    }

    try {
      return JSON.parse(result.stdout) as T;
    } catch (e: any) {
      this.outputChannel.log(`JSON parse error: ${e.message}. Raw output: ${result.stdout}`, 'ERROR');
      throw new Error(`Engram CLI returned malformed JSON: ${e.message}`);
    }
  }

  /**
   * Executes `engram -C <workspaceRoot> --json doctor` safely.
   */
  public async doctor(
    binaryPath: string,
    workspaceRoot: string,
    timeoutMs = 30000,
    cancellationToken?: vscode.CancellationToken
  ): Promise<DoctorReport> {
    if (this.isRunningHealthCheck) {
      throw new Error('A health check is already running. Please wait for it to complete.');
    }

    this.isRunningHealthCheck = true;
    try {
      this.outputChannel.log(`Invoking: ${binaryPath} -C "${workspaceRoot}" --json doctor`);
      const args = ['-C', workspaceRoot, '--json', 'doctor'];
      const parsed = await this.executeJson<any>(binaryPath, args, workspaceRoot, timeoutMs, cancellationToken);
      const validated = this.validateDoctorReport(parsed);
      return validated;
    } catch (err: any) {
      if (err.message && err.message.startsWith('Engram CLI failed:')) {
        throw new Error(err.message.replace('Engram CLI failed:', 'Engram Doctor failed:'));
      }
      throw err;
    } finally {
      this.isRunningHealthCheck = false;
    }
  }

  /**
   * Lists all durable engineering memories in the workspace with runtime validation.
   */
  public async listMemories(
    binaryPath: string,
    workspaceRoot: string,
    timeoutMs = 30000
  ): Promise<DurableMemory[]> {
    this.outputChannel.log(`Listing durable memories for: ${workspaceRoot}`);
    const args = ['-C', workspaceRoot, '--json', 'memory', 'list'];
    const res = await this.executeJson<any>(binaryPath, args, workspaceRoot, timeoutMs);
    return this.validateMemoryList(res);
  }

  /**
   * Compiles minimum sufficient context for a given task with runtime validation.
   */
  public async compileContext(
    binaryPath: string,
    workspaceRoot: string,
    task: string,
    tokenBudget = 1200,
    cancellationToken?: vscode.CancellationToken,
    timeoutMs = 30000
  ): Promise<ContextReport> {
    if (this.isRunningCompileContext) {
      throw new Error('A context compilation is already in progress. Please wait for it to complete or cancel it.');
    }

    this.isRunningCompileContext = true;
    try {
      const safeTimeout = typeof timeoutMs === 'number' && !isNaN(timeoutMs) && timeoutMs > 0 ? timeoutMs : 30000;
      this.outputChannel.log(`Compiling context for task: "${task}" (budget: ${tokenBudget}, timeout: ${safeTimeout}ms)`);
      const args = [
        '-C',
        workspaceRoot,
        '--json',
        'context',
        task,
        '--token-budget',
        String(tokenBudget),
        '--explain',
      ];
      const res = await this.executeJson<any>(binaryPath, args, workspaceRoot, safeTimeout, cancellationToken);
      return this.validateContextReport(res);
    } finally {
      this.isRunningCompileContext = false;
    }
  }

  /**
   * Runtime schema validator for DoctorReport.
   */
  public validateDoctorReport(data: any): DoctorReport {
    if (!data || typeof data !== 'object') {
      throw new Error('Doctor response is not a JSON object.');
    }

    if (typeof data.protocol_version !== 'string') {
      throw new Error('Missing or invalid "protocol_version" in doctor response.');
    }

    if (typeof data.total_memories !== 'number') {
      throw new Error('Missing or invalid "total_memories" in doctor response.');
    }

    if (typeof data.live_count !== 'number') {
      throw new Error('Missing or invalid "live_count" in doctor response.');
    }

    if (!Array.isArray(data.items)) {
      throw new Error('Missing or invalid "items" array in doctor response.');
    }

    if (typeof data.event_chain_status !== 'string') {
      throw new Error('Missing or invalid "event_chain_status" in doctor response.');
    }

    const items: DoctorItem[] = data.items.map((item: any, idx: number) => {
      if (!item || typeof item !== 'object') {
        throw new Error(`Doctor item at index ${idx} is not an object.`);
      }
      return {
        memory_id: String(item.memory_id || ''),
        title: String(item.title || ''),
        scope_level: String(item.scope_level || ''),
        scope_target: item.scope_target ? String(item.scope_target) : null,
        liveness: String(item.liveness || ''),
        diagnostic_message: String(item.diagnostic_message || ''),
        suggested_actions: Array.isArray(item.suggested_actions)
          ? item.suggested_actions.map(String)
          : [],
      };
    });

    return {
      protocol_version: data.protocol_version,
      total_memories: data.total_memories,
      live_count: data.live_count,
      dormant_count: Number(data.dormant_count || 0),
      stale_count: Number(data.stale_count || 0),
      superseded_count: Number(data.superseded_count || 0),
      archival_candidate_count: Number(data.archival_candidate_count || 0),
      items,
      event_chain_status: data.event_chain_status,
      event_chain_details: String(data.event_chain_details || ''),
    };
  }

  /**
   * Runtime schema validator for DurableMemory list.
   */
  public validateMemoryList(data: any): DurableMemory[] {
    if (!data || typeof data !== 'object') {
      throw new Error('Memory list response is not a valid JSON object or array.');
    }
    const rawList = Array.isArray(data) ? data : data.memories;
    if (!Array.isArray(rawList)) {
      throw new Error('Memory list response does not contain a "memories" array.');
    }
    return rawList.map((m: any, idx: number) => {
      if (!m || typeof m !== 'object') {
        throw new Error(`Memory item at index ${idx} is not an object.`);
      }
      return {
        id: String(m.id || ''),
        title: String(m.title || 'Untitled Decision'),
        statement: String(m.statement || ''),
        reason: String(m.reason || ''),
        accepted_by: String(m.accepted_by || 'Unknown'),
        accepted_at: String(m.accepted_at || new Date().toISOString()),
        scope: {
          level: String(m.scope?.level || 'GLOBAL'),
          target: m.scope?.target ? String(m.scope.target) : null,
          team: m.scope?.team ? String(m.scope.team) : null,
          modules: Array.isArray(m.scope?.modules) ? m.scope.modules.map(String) : [],
          files: Array.isArray(m.scope?.files) ? m.scope.files.map(String) : [],
          symbols: Array.isArray(m.scope?.symbols) ? m.scope.symbols.map(String) : [],
        },
        evidence_tier: m.evidence_tier ? String(m.evidence_tier) : undefined,
        lifecycle_state: String(m.lifecycle_state || 'DURABLE'),
        supersedes: m.supersedes ? String(m.supersedes) : null,
        superseded_by: m.superseded_by ? String(m.superseded_by) : null,
        tags: Array.isArray(m.tags) ? m.tags.map(String) : [],
        ratified_at_commit: m.ratified_at_commit ? String(m.ratified_at_commit) : null,
        ratified_by: m.ratified_by ? String(m.ratified_by) : null,
        ratified_at: m.ratified_at ? String(m.ratified_at) : null,
      };
    });
  }

  /**
   * Runtime schema validator for ContextReport.
   */
  public validateContextReport(data: any): ContextReport {
    if (!data || typeof data !== 'object') {
      throw new Error('Context report response is not a JSON object.');
    }
    if (typeof data.task !== 'string') {
      throw new Error('Missing or invalid "task" in context report response.');
    }
    if (!Array.isArray(data.files)) {
      throw new Error('Missing or invalid "files" array in context report response.');
    }

    const files = data.files.map((f: any, idx: number) => {
      if (!f || typeof f !== 'object') {
        throw new Error(`Context file at index ${idx} is not an object.`);
      }
      return {
        path: String(f.path || ''),
        score: typeof f.score === 'number' ? f.score : 0,
        level: typeof f.level === 'number' ? f.level : 0,
        provenance: Array.isArray(f.provenance) ? f.provenance.map(String) : [],
        content: String(f.content || ''),
      };
    });

    return {
      task: data.task,
      project: String(data.project || ''),
      revision: String(data.revision || ''),
      indexed_files: typeof data.indexed_files === 'number' ? data.indexed_files : 0,
      token_budget: typeof data.token_budget === 'number' ? data.token_budget : 1200,
      estimated_tokens: typeof data.estimated_tokens === 'number' ? data.estimated_tokens : 0,
      confidence: String(data.confidence || 'LOW'),
      constraints: Array.isArray(data.constraints) ? data.constraints.map(String) : [],
      relevant_memories: Array.isArray(data.relevant_memories) ? data.relevant_memories : [],
      cochange_alerts: Array.isArray(data.cochange_alerts) ? data.cochange_alerts.map(String) : [],
      ecosystem_guard: String(data.ecosystem_guard || ''),
      gaps: Array.isArray(data.gaps) ? data.gaps.map(String) : [],
      files,
      excluded_files: Array.isArray(data.excluded_files) ? data.excluded_files : [],
    };
  }

  /**
   * Secure process execution using child_process.spawn with argument vector,
   * shell: false, cancellation token support, and process tree termination.
   */
  public executeProcess(
    executablePath: string,
    args: string[],
    cwd: string,
    timeoutMs: number,
    cancellationToken?: vscode.CancellationToken
  ): Promise<CliExecutionResult> {
    return new Promise((resolve, reject) => {
      const startTime = Date.now();
      let stdout = '';
      let stderr = '';
      let hasCompleted = false;

      const child = cp.spawn(executablePath, args, {
        cwd,
        shell: false,
        windowsHide: true,
      });

      let timeoutHandle: NodeJS.Timeout | null = null;
      let cancellationListener: vscode.Disposable | null = null;
      let hasKilled = false;

      const cleanup = () => {
        if (timeoutHandle) {
          clearTimeout(timeoutHandle);
          timeoutHandle = null;
        }
        if (cancellationListener) {
          cancellationListener.dispose();
          cancellationListener = null;
        }
      };

      const killChild = () => {
        if (hasKilled || !child.pid) {
          return;
        }
        hasKilled = true;
        try {
          if (process.platform === 'win32') {
            // Directly terminate child process handle via Win32 TerminateProcess
            try {
              child.kill();
            } catch {
              // Ignore if already terminated
            }
            // Terminate any descendant process tree without shell invocation
            try {
              const killer = cp.spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], {
                windowsHide: true,
                stdio: 'ignore',
              });
              killer.on('error', () => {
                // Ignore if taskkill is unavailable or process already exited
              });
            } catch {
              // Ignore
            }
          } else {
            child.kill('SIGTERM');
            setTimeout(() => {
              try {
                child.kill('SIGKILL');
              } catch {
                // ignore
              }
            }, 500);
          }
        } catch {
          // Process may have already exited
        }
      };

      if (cancellationToken?.isCancellationRequested) {
        hasCompleted = true;
        killChild();
        cleanup();
        reject(new Error('Operation cancelled by user.'));
        return;
      }

      if (timeoutMs > 0) {
        timeoutHandle = setTimeout(() => {
          if (!hasCompleted) {
            hasCompleted = true;
            killChild();
            cleanup();
            reject(new Error(`Command timed out after ${timeoutMs}ms.`));
          }
        }, timeoutMs);
      }

      if (cancellationToken) {
        cancellationListener = cancellationToken.onCancellationRequested(() => {
          if (!hasCompleted) {
            hasCompleted = true;
            killChild();
            cleanup();
            reject(new Error('Operation cancelled by user.'));
          }
        });
      }

      child.stdout.on('data', (chunk) => {
        stdout += chunk.toString();
      });

      child.stderr.on('data', (chunk) => {
        stderr += chunk.toString();
      });

      child.on('error', (err) => {
        if (!hasCompleted) {
          hasCompleted = true;
          cleanup();
          reject(new Error(`Failed to spawn process '${executablePath}': ${err.message}`));
        }
      });

      child.on('close', (code) => {
        if (!hasCompleted) {
          hasCompleted = true;
          cleanup();
          resolve({
            stdout,
            stderr,
            exitCode: code ?? 0,
            durationMs: Date.now() - startTime,
          });
        }
      });
    });
  }
}
