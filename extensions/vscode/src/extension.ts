import * as vscode from 'vscode';
import { BinaryResolver } from './binaryResolver';
import { EngramClient } from './engramClient';
import { EngramOutputChannel } from './outputChannel';
import { EngramStatusBar } from './statusBar';

let outputChannel: EngramOutputChannel | null = null;
let binaryResolver: BinaryResolver | null = null;
let engramClient: EngramClient | null = null;
let statusBar: EngramStatusBar | null = null;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = new EngramOutputChannel();
  binaryResolver = new BinaryResolver();
  engramClient = new EngramClient(outputChannel);
  statusBar = new EngramStatusBar();

  context.subscriptions.push(outputChannel);
  context.subscriptions.push(statusBar);

  outputChannel.log('Engram extension activated.');

  // Lazy binary discovery (does NOT run doctor, following Safeguard 1)
  const config = vscode.workspace.getConfiguration('engram');
  const explicitPath = config.get<string>('binaryPath', '');
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

  const initialResolution = await binaryResolver.resolve(explicitPath, workspaceRoot);
  statusBar.updateWithResolution(initialResolution);

  if (initialResolution.valid) {
    outputChannel.log(`Engram binary detected: ${initialResolution.path} (v${initialResolution.version}, source: ${initialResolution.source})`);
  } else {
    outputChannel.log(`Engram binary not resolved: ${initialResolution.error}`, 'WARN');
  }

  // 1. Command: engram.locateBinary
  const locateBinaryCommand = vscode.commands.registerCommand('engram.locateBinary', async () => {
    if (!binaryResolver || !statusBar || !outputChannel) {
      return;
    }

    outputChannel.log('Triggering binary discovery...');
    binaryResolver.clearCache();

    const cfg = vscode.workspace.getConfiguration('engram');
    const exp = cfg.get<string>('binaryPath', '');
    const wsRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

    const resolution = await binaryResolver.resolve(exp, wsRoot);
    statusBar.updateWithResolution(resolution);

    if (resolution.valid) {
      outputChannel.log(`Binary successfully located at: ${resolution.path} (v${resolution.version})`);
      vscode.window.showInformationMessage(
        `Engram binary located: ${resolution.path} (v${resolution.version})`
      );
    } else {
      outputChannel.log(`Binary resolution failed: ${resolution.error}`, 'WARN');
      const action = await vscode.window.showWarningMessage(
        `Engram Binary Missing or Incompatible: ${resolution.error}`,
        'Install Engram',
        'Configure Path',
        'Retry'
      );

      if (action === 'Install Engram') {
        vscode.env.openExternal(vscode.Uri.parse('https://github.com/code-amalps/Engram#installation'));
      } else if (action === 'Configure Path') {
        vscode.commands.executeCommand('workbench.action.openSettings', 'engram.binaryPath');
      } else if (action === 'Retry') {
        vscode.commands.executeCommand('engram.locateBinary');
      }
    }
  });

  // 2. Command: engram.checkHealth (Doctor)
  const checkHealthCommand = vscode.commands.registerCommand('engram.checkHealth', async () => {
    if (!binaryResolver || !engramClient || !statusBar || !outputChannel) {
      return;
    }

    const cfg = vscode.workspace.getConfiguration('engram');
    const exp = cfg.get<string>('binaryPath', '');
    const timeoutMs = cfg.get<number>('timeoutMs', 30000);
    const wsFolder = vscode.workspace.workspaceFolders?.[0];

    if (!wsFolder) {
      vscode.window.showWarningMessage('Please open a workspace folder to run Engram Doctor.');
      return;
    }

    const wsRoot = wsFolder.uri.fsPath;
    const resolution = await binaryResolver.resolve(exp, wsRoot);

    if (!resolution.valid) {
      statusBar.updateWithResolution(resolution);
      const action = await vscode.window.showErrorMessage(
        `Cannot run Engram Doctor: ${resolution.error}`,
        'Locate Binary',
        'Configure Path'
      );
      if (action === 'Locate Binary') {
        vscode.commands.executeCommand('engram.locateBinary');
      } else if (action === 'Configure Path') {
        vscode.commands.executeCommand('workbench.action.openSettings', 'engram.binaryPath');
      }
      return;
    }

    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: 'Engram',
        cancellable: true,
      },
      async (progress, token) => {
        progress.report({ message: 'Running health diagnosis...' });

        try {
          const report = await engramClient!.doctor(resolution.path, wsRoot, timeoutMs, token);
          
          statusBar!.setHealthResult(
            report.total_memories,
            report.live_count,
            report.event_chain_status === 'EVENT_CHAIN_VALID'
          );

          outputChannel!.log('=== ENGRAM DOCTOR DIAGNOSTIC REPORT ===');
          outputChannel!.log(`Protocol Version:     ${report.protocol_version}`);
          outputChannel!.log(`Total Memories:       ${report.total_memories}`);
          outputChannel!.log(`Live Decisions:       ${report.live_count}`);
          outputChannel!.log(`Dormant Decisions:    ${report.dormant_count}`);
          outputChannel!.log(`Stale Review Alerts:  ${report.stale_count}`);
          outputChannel!.log(`Archival Candidates:  ${report.archival_candidate_count}`);
          outputChannel!.log(`Provenance Chain:     ${report.event_chain_status}`);
          outputChannel!.log(`Chain Details:        ${report.event_chain_details}`);

          if (report.items.length > 0) {
            outputChannel!.log('--- Memory Health Items ---');
            for (const item of report.items) {
              outputChannel!.log(`[${item.liveness}] ${item.title} (${item.memory_id}) — ${item.diagnostic_message}`);
            }
          }

          const action = await vscode.window.showInformationMessage(
            `Engram Health: OK — ${report.total_memories} memories (${report.live_count} live), Chain: ${report.event_chain_status}`,
            'View Output'
          );

          if (action === 'View Output') {
            outputChannel!.show();
          }
        } catch (err: any) {
          statusBar!.setError(err.message);
          outputChannel!.log(`Doctor diagnosis failed: ${err.message}`, 'ERROR');

          const action = await vscode.window.showErrorMessage(
            `Engram Doctor Failed: ${err.message}`,
            'View Output'
          );

          if (action === 'View Output') {
            outputChannel!.show();
          }
        }
      }
    );
  });

  // 3. Command: engram.showOutputChannel
  const showOutputChannelCommand = vscode.commands.registerCommand('engram.showOutputChannel', () => {
    outputChannel?.show();
  });

  context.subscriptions.push(locateBinaryCommand);
  context.subscriptions.push(checkHealthCommand);
  context.subscriptions.push(showOutputChannelCommand);

  // Watch for configuration changes
  const configWatcher = vscode.workspace.onDidChangeConfiguration((e) => {
    if (e.affectsConfiguration('engram.binaryPath')) {
      outputChannel?.log('Configuration engram.binaryPath changed. Refreshing discovery...');
      vscode.commands.executeCommand('engram.locateBinary');
    }
  });

  context.subscriptions.push(configWatcher);
}

export function deactivate(): void {
  if (outputChannel) {
    outputChannel.log('Engram extension deactivated.');
    outputChannel.dispose();
  }
}
