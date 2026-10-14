import * as cp from 'child_process';
import * as vscode from 'vscode';
import { DoctorReport, DoctorItem } from './types';
import { EngramOutputChannel } from './outputChannel';

export interface CliExecutionResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  durationMs: number;
}

export class EngramClient {
  private isRunningHealthCheck = false;

  constructor(private outputChannel: EngramOutputChannel) {}

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
    const startTime = Date.now();

    try {
      this.outputChannel.log(`Invoking: ${binaryPath} -C "${workspaceRoot}" --json doctor`);
      
      const args = ['-C', workspaceRoot, '--json', 'doctor'];
      const result = await this.executeProcess(binaryPath, args, workspaceRoot, timeoutMs, cancellationToken);
      const durationMs = Date.now() - startTime;

      this.outputChannel.log(`Doctor completed in ${durationMs}ms with exit code ${result.exitCode}`);

      if (result.exitCode !== 0) {
        const errorMsg = result.stderr.trim() || result.stdout.trim() || `Process exited with code ${result.exitCode}`;
        this.outputChannel.log(`Doctor error: ${errorMsg}`, 'ERROR');
        throw new Error(`Engram Doctor failed: ${errorMsg}`);
      }

      if (!result.stdout || result.stdout.trim() === '') {
        throw new Error('Engram Doctor returned empty output.');
      }

      let parsed: any;
      try {
        parsed = JSON.parse(result.stdout);
      } catch (e: any) {
        this.outputChannel.log(`JSON parse error: ${e.message}. Raw output: ${result.stdout}`, 'ERROR');
        throw new Error(`Engram Doctor returned malformed JSON: ${e.message}`);
      }

      const validated = this.validateDoctorReport(parsed);
      return validated;
    } finally {
      this.isRunningHealthCheck = false;
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
            cp.exec(`taskkill /pid ${child.pid} /T /F`, () => {});
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
