import * as assert from 'assert';
import * as vscode from 'vscode';
import { activate, deactivate } from '../src/extension';

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

    // Verify command registrations
    assert.ok(registeredCommands.has('engram.locateBinary'), 'engram.locateBinary should be registered');
    assert.ok(registeredCommands.has('engram.checkHealth'), 'engram.checkHealth should be registered');
    assert.ok(registeredCommands.has('engram.showOutputChannel'), 'engram.showOutputChannel should be registered');

    // Verify context subscriptions contains status bar, output channel, commands, watcher
    assert.ok(subscriptions.length >= 5, 'Should register all disposables in context subscriptions');
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

  it('should deactivate cleanly', () => {
    assert.doesNotThrow(() => {
      deactivate();
    });
  });
});
