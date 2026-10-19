import * as vscode from 'vscode';
import { BinaryResolutionResult } from './types';

export class EngramStatusBar {
  private statusBarItem: vscode.StatusBarItem;

  constructor() {
    this.statusBarItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Right,
      100
    );
    this.setPending();
    this.statusBarItem.show();
  }

  public setPending(): void {
    this.statusBarItem.text = '$(sync~spin) Engram: Initializing...';
    this.statusBarItem.tooltip = 'Engram binary discovery in progress';
    this.statusBarItem.command = 'engram.locateBinary';
    this.statusBarItem.backgroundColor = undefined;
  }

  public updateWithResolution(resolution: BinaryResolutionResult): void {
    if (resolution.valid) {
      this.statusBarItem.text = `$(shield) Engram v${resolution.version}`;
      this.statusBarItem.tooltip = new vscode.MarkdownString(
        `**Engram Active**\n\n- Version: \`${resolution.version}\`\n- Path: \`${resolution.path}\`\n- Source: \`${resolution.source}\`\n\nClick to run health check.`
      );
      this.statusBarItem.command = 'engram.checkHealth';
      this.statusBarItem.backgroundColor = undefined;
    } else {
      this.statusBarItem.text = '$(alert) Engram: Not Found';
      this.statusBarItem.tooltip = new vscode.MarkdownString(
        `**Engram Binary Missing or Incompatible**\n\n${resolution.error}\n\nClick to locate or configure binary.`
      );
      this.statusBarItem.command = 'engram.locateBinary';
      this.statusBarItem.backgroundColor = new vscode.ThemeColor(
        'statusBarItem.warningBackground'
      );
    }
  }

  public setError(message: string): void {
    this.statusBarItem.text = '$(error) Engram: Error';
    this.statusBarItem.tooltip = `Engram Error: ${message}. Click to check health.`;
    this.statusBarItem.command = 'engram.checkHealth';
    this.statusBarItem.backgroundColor = new vscode.ThemeColor(
      'statusBarItem.errorBackground'
    );
  }

  public setHealthResult(totalMemories: number, liveCount: number, eventChainValid: boolean): void {
    const chainSymbol = eventChainValid ? '✓' : '⚠️';
    this.statusBarItem.tooltip = new vscode.MarkdownString(
      `**Engram Health: OK**\n\n- Total Memories: ${totalMemories}\n- Active / Live: ${liveCount}\n- Provenance Hash Chain: ${chainSymbol}\n\nClick to run health check again.`
    );
  }

  public updateWithSetupReport(report: import('./types').SetupDiagnosticReport): void {
    if (report.state === 'READY') {
      const version = report.binary.valid ? ` v${report.binary.version}` : '';
      const toolInfo = report.health?.toolCount ? ` (${report.health.toolCount} tools)` : '';
      this.statusBarItem.text = `$(shield) Engram: Ready${version}`;
      this.statusBarItem.tooltip = new vscode.MarkdownString(
        `**Engram: Ready for AI Agents**\n\n- Binary: \`${report.binary.valid ? report.binary.path : 'N/A'}\`\n- MCP Server: Active (${report.registration.path})\n- Governed Tools: ${report.health?.toolCount || 15} discovered\n- Workspace Projects: ${report.workspace?.detectedProjects.join(', ') || 'N/A'}\n\nClick to check setup details.`
      );
      this.statusBarItem.command = 'engram.checkSetup';
      this.statusBarItem.backgroundColor = undefined;
    } else if (report.state === 'SETUP_REQUIRED') {
      this.statusBarItem.text = '$(tools) Engram: Setup Required';
      const reason = !report.binary.valid
        ? 'Engram CLI is not installed or not in PATH.'
        : 'MCP server is not registered in this workspace.';
      this.statusBarItem.tooltip = new vscode.MarkdownString(
        `**Engram Setup Required**\n\n${reason}\n\nClick for 1-Click Setup & Repair.`
      );
      this.statusBarItem.command = 'engram.fixSetup';
      this.statusBarItem.backgroundColor = new vscode.ThemeColor(
        'statusBarItem.warningBackground'
      );
    } else if (report.state === 'STARTING') {
      this.statusBarItem.text = '$(sync~spin) Engram: Checking...';
      this.statusBarItem.tooltip = 'Running Engram setup diagnostics and MCP handshake';
      this.statusBarItem.command = 'engram.checkSetup';
      this.statusBarItem.backgroundColor = undefined;
    } else {
      // ERROR
      this.statusBarItem.text = '$(error) Engram: Setup Error';
      const detail =
        report.registration.details ||
        report.health?.error ||
        (!report.binary.valid ? report.binary.error : 'Setup error occurred.');
      this.statusBarItem.tooltip = new vscode.MarkdownString(
        `**Engram Setup Error**\n\n${detail}\n\nClick to repair setup.`
      );
      this.statusBarItem.command = 'engram.fixSetup';
      this.statusBarItem.backgroundColor = new vscode.ThemeColor(
        'statusBarItem.errorBackground'
      );
    }
  }

  public dispose(): void {
    this.statusBarItem.dispose();
  }
}
