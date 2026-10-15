import * as vscode from 'vscode';
import { BinaryResolver } from './binaryResolver';
import { EngramClient } from './engramClient';
import { EngramOutputChannel } from './outputChannel';
import { EngramStatusBar } from './statusBar';
import { MemoryTreeProvider } from './treeViews';
import {
  handleCompileContext,
  handleCopyContext,
  formatMemoryToMarkdown,
} from './contextCompiler';
import { DurableMemory, ContextReport } from './types';

let outputChannel: EngramOutputChannel | null = null;
let binaryResolver: BinaryResolver | null = null;
let engramClient: EngramClient | null = null;
let statusBar: EngramStatusBar | null = null;
let memoryTreeProvider: MemoryTreeProvider | null = null;
const lastCompiledReportRef: { current: ContextReport | null } = { current: null };

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = new EngramOutputChannel();
  binaryResolver = new BinaryResolver();
  engramClient = new EngramClient(outputChannel);
  statusBar = new EngramStatusBar();
  memoryTreeProvider = new MemoryTreeProvider(engramClient);

  context.subscriptions.push(outputChannel);
  context.subscriptions.push(statusBar);

  // Register Memory Tree View for Activity Bar Engram Explorer
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider('engram.views.memories', memoryTreeProvider)
  );

  outputChannel.log('Engram extension activated.');

  // Helper to get active workspace folder root
  const getWorkspaceRoot = (): string | undefined => {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  };

  // Helper to resolve binary with current settings
  const resolveCurrentBinary = async () => {
    if (!binaryResolver) {
      return null;
    }
    const cfg = vscode.workspace.getConfiguration('engram');
    const exp = cfg.get<string>('binaryPath', '');
    const wsRoot = getWorkspaceRoot();
    const resolution = await binaryResolver.resolve(exp, wsRoot);
    statusBar?.updateWithResolution(resolution);
    memoryTreeProvider?.setResolution(resolution, wsRoot);
    return { resolution, wsRoot };
  };

  // Initial lazy discovery
  const initial = await resolveCurrentBinary();
  if (initial) {
    if (initial.resolution.valid) {
      outputChannel.log(
        `Engram binary detected: ${initial.resolution.path} (v${initial.resolution.version}, source: ${initial.resolution.source})`
      );
    } else {
      outputChannel.log(`Engram binary not resolved: ${initial.resolution.error}`, 'WARN');
    }
  }

  // 1. Command: engram.locateBinary
  const locateBinaryCommand = vscode.commands.registerCommand('engram.locateBinary', async () => {
    if (!binaryResolver || !outputChannel) {
      return;
    }

    outputChannel.log('Triggering binary discovery...');
    binaryResolver.clearCache();

    const result = await resolveCurrentBinary();
    if (!result) {
      return;
    }

    const { resolution } = result;
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

    const wsRoot = getWorkspaceRoot();
    if (!wsRoot) {
      vscode.window.showWarningMessage('Please open a workspace folder to run Engram Doctor.');
      return;
    }

    const cfg = vscode.workspace.getConfiguration('engram');
    const exp = cfg.get<string>('binaryPath', '');
    const timeoutMs = cfg.get<number>('timeoutMs', 30000);
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

    let report: any;
    let doctorError: any;

    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: 'Engram',
        cancellable: true,
      },
      async (progress, token) => {
        progress.report({ message: 'Running health diagnosis...' });

        try {
          report = await engramClient!.doctor(resolution.path, wsRoot, timeoutMs, token);
        } catch (err: any) {
          doctorError = err;
        }
      }
    );

    if (doctorError) {
      statusBar!.setError(doctorError.message);
      outputChannel!.log(`Doctor diagnosis failed: ${doctorError.message}`, 'ERROR');

      const action = await vscode.window.showErrorMessage(
        `Engram Doctor Failed: ${doctorError.message}`,
        'View Output'
      );

      if (action === 'View Output') {
        outputChannel!.show();
      }
      return;
    }

    if (!report) {
      return;
    }

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
  });

  // 3. Command: engram.showOutputChannel
  const showOutputChannelCommand = vscode.commands.registerCommand('engram.showOutputChannel', () => {
    outputChannel?.show();
  });

  // 4. Command: engram.refreshMemories
  const refreshMemoriesCommand = vscode.commands.registerCommand('engram.refreshMemories', () => {
    memoryTreeProvider?.refresh();
  });

  // 5. Command: engram.showMemoryDetails
  const showMemoryDetailsCommand = vscode.commands.registerCommand(
    'engram.showMemoryDetails',
    async (memory: DurableMemory) => {
      if (!memory) {
        return;
      }
      const markdown = formatMemoryToMarkdown(memory);
      const doc = await vscode.workspace.openTextDocument({
        content: markdown,
        language: 'markdown',
      });
      await vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Beside });
    }
  );

  // 6. Command: engram.compileContext
  const compileContextCommand = vscode.commands.registerCommand('engram.compileContext', async () => {
    if (!binaryResolver || !engramClient || !outputChannel) {
      return;
    }
    await handleCompileContext(engramClient, outputChannel, binaryResolver, lastCompiledReportRef);
  });

  // 7. Command: engram.copyContext
  const copyContextCommand = vscode.commands.registerCommand('engram.copyContext', async () => {
    if (!outputChannel) {
      return;
    }
    if (lastCompiledReportRef.current) {
      await handleCopyContext(lastCompiledReportRef.current, outputChannel);
    } else {
      const action = await vscode.window.showInformationMessage(
        'No context has been compiled yet. Would you like to compile context for a task now?',
        'Compile Context'
      );
      if (action === 'Compile Context') {
        vscode.commands.executeCommand('engram.compileContext');
      }
    }
  });

  context.subscriptions.push(locateBinaryCommand);
  context.subscriptions.push(checkHealthCommand);
  context.subscriptions.push(showOutputChannelCommand);
  context.subscriptions.push(refreshMemoriesCommand);
  context.subscriptions.push(showMemoryDetailsCommand);
  context.subscriptions.push(compileContextCommand);
  context.subscriptions.push(copyContextCommand);

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
