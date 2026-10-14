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

  public dispose(): void {
    this.statusBarItem.dispose();
  }
}
