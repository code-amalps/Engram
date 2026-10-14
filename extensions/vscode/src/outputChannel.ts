import * as vscode from 'vscode';

export class EngramOutputChannel {
  private channel: vscode.OutputChannel;

  constructor() {
    this.channel = vscode.window.createOutputChannel('Engram');
  }

  public log(message: string, level: 'INFO' | 'WARN' | 'ERROR' = 'INFO'): void {
    const timestamp = new Date().toISOString();
    const sanitized = this.sanitize(message);
    this.channel.appendLine(`[${timestamp}] [${level}] ${sanitized}`);
  }

  public show(preserveFocus = true): void {
    this.channel.show(preserveFocus);
  }

  public clear(): void {
    this.channel.clear();
  }

  public dispose(): void {
    this.channel.dispose();
  }

  /**
   * Redacts sensitive patterns such as tokens, keys, and passwords.
   */
  public sanitize(input: string): string {
    return input
      .replace(/(?:sk-[A-Za-z0-9_-]{20,})/g, '[REDACTED_API_KEY]')
      .replace(/(?:ghp_[A-Za-z0-9]{30,})/g, '[REDACTED_GITHUB_TOKEN]')
      .replace(/(?:password=)[^\s&]+/gi, 'password=[REDACTED]')
      .replace(/(?:Bearer\s+)[A-Za-z0-9._~+/-]+=*/gi, 'Bearer [REDACTED]');
  }
}
