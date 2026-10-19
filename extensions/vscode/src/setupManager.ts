import { BinaryResolver } from './binaryResolver';
import { WorkspaceDetector } from './workspaceDetector';
import { McpRegistrationManager } from './mcpRegistration';
import { McpClient } from './mcpClient';
import { SetupInstructionsManager } from './setupInstructions';
import { EngramOutputChannel } from './outputChannel';
import {
  SetupDiagnosticReport,
  SetupState,
  BinaryResolutionResult,
} from './types';

export class SetupManager {
  constructor(
    private readonly binaryResolver: BinaryResolver,
    private readonly workspaceDetector: WorkspaceDetector,
    private readonly mcpRegistration: McpRegistrationManager,
    private readonly mcpClient: McpClient,
    private readonly instructionsManager: SetupInstructionsManager,
    private readonly outputChannel?: EngramOutputChannel
  ) {}

  /**
   * Evaluates the complete setup pipeline and returns a structured diagnostic report.
   */
  public async checkSetup(
    workspaceRoot?: string,
    options: { runHandshake?: boolean; explicitBinary?: string } = { runHandshake: true }
  ): Promise<SetupDiagnosticReport> {
    const wsRoot = workspaceRoot;

    // Stage 1: Binary resolution & compatibility
    const binaryRes: BinaryResolutionResult = await this.binaryResolver.resolve(
      options.explicitBinary,
      wsRoot
    );

    // Stage 2: Workspace detection
    const workspaceInfo = wsRoot
      ? this.workspaceDetector.detect(wsRoot)
      : undefined;

    // Stage 3: MCP registration inspection
    const registration = wsRoot
      ? this.mcpRegistration.inspect(wsRoot, binaryRes.valid ? binaryRes.path : undefined)
      : {
          status: 'NOT_CONFIGURED' as const,
          path: '',
          details: 'No workspace opened.',
        };

    // Stage 4: MCP process startup & tool handshake
    let health: any = undefined;
    if (
      options.runHandshake &&
      binaryRes.valid &&
      wsRoot &&
      registration.status === 'CONFIGURED_HEALTHY'
    ) {
      try {
        health = await this.mcpClient.checkHealth(binaryRes.path, wsRoot, 6000);
      } catch (err: any) {
        health = {
          success: false,
          latencyMs: 0,
          toolCount: 0,
          tools: [],
          protocolVersion: '',
          serverName: '',
          error: err.message || String(err),
        };
      }
    }

    // Determine overall state and recommended action
    let state: SetupState = 'READY';
    let recommendedAction: SetupDiagnosticReport['recommendedAction'] = 'none';

    if (!binaryRes.valid) {
      if (binaryRes.isExplicitConfigError) {
        state = 'ERROR';
        recommendedAction = 'install_binary';
      } else {
        state = 'SETUP_REQUIRED';
        recommendedAction = 'install_binary';
      }
    } else if (!wsRoot) {
      state = 'SETUP_REQUIRED';
      recommendedAction = 'none';
    } else if (registration.status === 'CONFLICT') {
      state = 'ERROR';
      recommendedAction = 'repair_conflict';
    } else if (registration.status === 'MALFORMED_JSON') {
      state = 'ERROR';
      recommendedAction = 'repair_conflict';
    } else if (registration.status === 'NOT_CONFIGURED') {
      state = 'SETUP_REQUIRED';
      recommendedAction = 'register_mcp';
    } else if (health && !health.success) {
      state = 'ERROR';
      recommendedAction = 'register_mcp';
    } else if (health && health.success) {
      state = 'READY';
      recommendedAction = 'none';
    } else {
      // Configured healthy, no handshake run
      state = 'READY';
      recommendedAction = 'none';
    }

    const report: SetupDiagnosticReport = {
      state,
      workspace: workspaceInfo,
      binary: binaryRes,
      registration,
      health,
      recommendedAction,
    };

    if (this.outputChannel) {
      this.logReport(report);
    }

    return report;
  }

  /**
   * One-click safe repair: resolves binary, registers MCP server, and verifies handshake.
   */
  public async fixSetup(
    workspaceRoot: string,
    options: { force?: boolean; explicitBinary?: string } = {}
  ): Promise<{ success: boolean; message: string; report: SetupDiagnosticReport }> {
    this.outputChannel?.log(`[Setup] Starting 1-Click Repair for workspace: ${workspaceRoot}`);

    // Step 1: Resolve binary
    const binaryRes = await this.binaryResolver.resolve(options.explicitBinary, workspaceRoot);
    if (!binaryRes.valid) {
      const msg = `Cannot complete setup: Engram binary not found or incompatible (${binaryRes.error}). Please install Engram.`;
      this.outputChannel?.log(`[Setup] ${msg}`, 'ERROR');
      const report = await this.checkSetup(workspaceRoot, { runHandshake: false, explicitBinary: options.explicitBinary });
      return { success: false, message: msg, report };
    }

    // Step 2: Register MCP in .vscode/mcp.json
    const regResult = this.mcpRegistration.register(
      workspaceRoot,
      binaryRes.path,
      options.force ?? false
    );
    if (!regResult.success) {
      const msg = `Failed to register MCP server in .vscode/mcp.json: ${regResult.error}`;
      this.outputChannel?.log(`[Setup] ${msg}`, 'ERROR');
      const report = await this.checkSetup(workspaceRoot, { runHandshake: false, explicitBinary: options.explicitBinary });
      return { success: false, message: msg, report };
    }

    // Step 3: Run live handshake verification
    const health = await this.mcpClient.checkHealth(binaryRes.path, workspaceRoot, 6000);
    const finalReport = await this.checkSetup(workspaceRoot, { runHandshake: false, explicitBinary: options.explicitBinary });
    finalReport.health = health;

    if (!health.success) {
      const msg = `MCP configuration registered, but server handshake failed: ${health.error}`;
      finalReport.state = 'ERROR';
      this.outputChannel?.log(`[Setup] ${msg}`, 'ERROR');
      return { success: false, message: msg, report: finalReport };
    }

    finalReport.state = 'READY';
    const msg = `Engram MCP server successfully configured and verified (${health.toolCount} tools active, ${health.latencyMs}ms).`;
    this.outputChannel?.log(`[Setup] ${msg}`, 'INFO');
    return { success: true, message: msg, report: finalReport };
  }

  private logReport(report: SetupDiagnosticReport): void {
    if (!this.outputChannel) {
      return;
    }
    this.outputChannel.log('=== ENGRAM AGENT SETUP DIAGNOSTIC REPORT ===');
    this.outputChannel.log(`Overall State:        ${report.state}`);
    this.outputChannel.log(
      `Binary:               ${report.binary.valid ? `v${report.binary.version} at ${report.binary.path}` : `INVALID (${report.binary.error})`}`
    );
    if (report.workspace) {
      this.outputChannel.log(`Workspace Root:       ${report.workspace.root}`);
      this.outputChannel.log(`Detected Projects:    ${report.workspace.detectedProjects.join(', ')}`);
      this.outputChannel.log(`Initialized (.engram): ${report.workspace.isInitialized}`);
    }
    this.outputChannel.log(`MCP Registration:     ${report.registration.status} (${report.registration.path})`);
    if (report.registration.details) {
      this.outputChannel.log(`Registration Detail:  ${report.registration.details}`);
    }
    if (report.health) {
      this.outputChannel.log(
        `MCP Handshake:        ${report.health.success ? `SUCCESS (${report.health.toolCount} tools, ${report.health.latencyMs}ms)` : `FAILED (${report.health.error})`}`
      );
      if (report.health.tools.length > 0) {
        this.outputChannel.log(`Governed Tools:       ${report.health.tools.join(', ')}`);
      }
    }
    this.outputChannel.log(`Recommended Action:   ${report.recommendedAction}`);
    this.outputChannel.log('============================================');
  }
}
