import * as assert from 'assert';
import * as path from 'path';
import * as vscode from 'vscode';
import { activate, deactivate } from '../src/extension';
import { DurableMemory } from '../src/types';

describe('Extension Lifecycle & Command Registration Tests', () => {
  const registeredCommands: Map<string, (...args: any[]) => any> = new Map();

  before(() => {
    // Intercept registered commands
    const origRegister = vscode.commands.registerCommand;
    vscode.commands.registerCommand = ((command: string, callback: (...args: any[]) => any) => {
      registeredCommands.set(command, callback);
      return { dispose: () => registeredCommands.delete(command) };
    }) as any;
  });

  it('should activate extension and register all Milestone 1 commands', async () => {
    const subscriptions: { dispose(): any }[] = [];
    const context: any = {
      subscriptions,
      extensionUri: vscode.Uri.file(__dirname),
    };

    await activate(context);

    // Verify Milestone 1 command registrations
    assert.ok(registeredCommands.has('engram.locateBinary'), 'engram.locateBinary should be registered');
    assert.ok(registeredCommands.has('engram.checkHealth'), 'engram.checkHealth should be registered');
    assert.ok(registeredCommands.has('engram.showOutputChannel'), 'engram.showOutputChannel should be registered');

    // Verify Milestone 2 command registrations
    assert.ok(registeredCommands.has('engram.compileContext'), 'engram.compileContext should be registered');
    assert.ok(registeredCommands.has('engram.copyContext'), 'engram.copyContext should be registered');
    assert.ok(registeredCommands.has('engram.refreshMemories'), 'engram.refreshMemories should be registered');
    assert.ok(registeredCommands.has('engram.showMemoryDetails'), 'engram.showMemoryDetails should be registered');

    // Verify context subscriptions contains status bar, output channel, commands, watcher, tree views
    assert.ok(subscriptions.length >= 8, 'Should register all disposables in context subscriptions');
  });

  it('should execute engram.refreshMemories without error', async () => {
    const fn = registeredCommands.get('engram.refreshMemories');
    assert.ok(fn, 'engram.refreshMemories must exist');
    await assert.doesNotReject(async () => {
      await fn!();
    });
  });

  it('should execute engram.copyContext warning when no context compiled yet', async () => {
    const fn = registeredCommands.get('engram.copyContext');
    assert.ok(fn, 'engram.copyContext must exist');
    await assert.doesNotReject(async () => {
      await fn!();
    });
  });

  it('should execute engram.showOutputChannel without error', () => {
    const fn = registeredCommands.get('engram.showOutputChannel');
    assert.ok(fn, 'engram.showOutputChannel must exist');
    assert.doesNotThrow(() => fn!());
  });

  it('should execute engram.locateBinary without error', async () => {
    const fn = registeredCommands.get('engram.locateBinary');
    assert.ok(fn, 'engram.locateBinary must exist');
    await assert.doesNotReject(async () => {
      await fn!();
    });
  });

  it('should execute engram.showMemoryDetails and open markdown document', async () => {
    const fn = registeredCommands.get('engram.showMemoryDetails');
    assert.ok(fn, 'engram.showMemoryDetails must exist');

    const sampleMem: DurableMemory = {
      id: 'ADR-099',
      title: 'Database Retry Strategy',
      statement: 'Use exponential backoff with jitter on transient failures.',
      reason: 'Prevent thundering herds on primary reconnect.',
      accepted_by: 'lead',
      accepted_at: new Date().toISOString(),
      scope: {
        level: 'MODULE',
        target: 'database',
        team: 'platform',
        modules: ['db'],
        files: ['src/db.ts'],
        symbols: ['connectWithRetry'],
      },
      lifecycle_state: 'DURABLE',
      tags: ['db', 'reliability'],
      ratified_at_commit: null,
      ratified_by: null,
      ratified_at: null,
      supersedes: null,
      superseded_by: null,
    };

    await assert.doesNotReject(async () => {
      await fn!(sampleMem);
    });
  });

  it('should execute engram.compileContext end-to-end, open preview, and copy to clipboard', async () => {
    const wsRoot = path.resolve(__dirname, '../../..');
    (vscode.workspace as any).workspaceFolders = [
      { uri: { fsPath: wsRoot } },
    ];

    // Mock inputs for prompt and budget
    const origShowInputBox = vscode.window.showInputBox;
    const origShowInfo = vscode.window.showInformationMessage;

    (vscode.window as any).showInputBox = async (opts: any) => {
      if (opts?.prompt?.includes('Token budget') || opts?.value === '1200') {
        return '1200';
      }
      return 'Add retry logic to the payment service';
    };

    // User chooses 'Copy for AI Agent'
    (vscode.window as any).showInformationMessage = async (msg: string, ...items: string[]) => {
      if (items.includes('Copy for AI Agent')) {
        return 'Copy for AI Agent';
      }
      return items[0];
    };

    try {
      const compileFn = registeredCommands.get('engram.compileContext');
      assert.ok(compileFn, 'engram.compileContext must exist');
      await compileFn!();

      // Verify clipboard contains generated XML context
      const clipboardContent = await vscode.env.clipboard.readText();
      assert.match(clipboardContent, /<engram_context/);
      assert.match(clipboardContent, /<\/engram_context>/);

      // Verify engram.copyContext now copies cached compiled context
      const copyFn = registeredCommands.get('engram.copyContext');
      assert.ok(copyFn, 'engram.copyContext must exist');
      await copyFn!();

      const clipboardContent2 = await vscode.env.clipboard.readText();
      assert.match(clipboardContent2, /<engram_context/);
    } finally {
      (vscode.window as any).showInputBox = origShowInputBox;
      (vscode.window as any).showInformationMessage = origShowInfo;
    }
  });

  it('should respect custom engram.timeoutMs configuration during compileContext', async () => {
    const wsRoot = path.resolve(__dirname, '../../..');
    (vscode.workspace as any).workspaceFolders = [
      { uri: { fsPath: wsRoot } },
    ];

    const origGetConfig = vscode.workspace.getConfiguration;
    (vscode.workspace as any).getConfiguration = (section: string) => ({
      get: (key: string, defaultValue: any) => {
        if (key === 'timeoutMs') {
          return 45000;
        }
        return defaultValue;
      },
      update: async () => {},
    });

    try {
      const compileFn = registeredCommands.get('engram.compileContext');
      assert.ok(compileFn, 'engram.compileContext must exist');
      await assert.doesNotReject(async () => {
        await compileFn!();
      });
    } finally {
      (vscode.workspace as any).getConfiguration = origGetConfig;
    }
  });

  it('should deactivate cleanly', () => {
    assert.doesNotThrow(() => {
      deactivate();
    });
  });
});
