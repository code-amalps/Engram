import * as assert from 'assert';
import * as vscode from 'vscode';
import { MemoryTreeProvider, MemoryTreeItem } from '../src/treeViews';
import { EngramClient } from '../src/engramClient';
import { DurableMemory, BinaryResolutionResult } from '../src/types';

describe('MemoryTreeProvider Tests', () => {
  let mockClient: any;
  let provider: MemoryTreeProvider;

  const sampleMemories: DurableMemory[] = [
    {
      id: 'DEC-001',
      title: 'Global Auth Standard',
      statement: 'Use JWT for all service auth.',
      reason: 'Stateless and scalable across microservices.',
      accepted_by: 'alice',
      accepted_at: '2026-10-01T12:00:00Z',
      scope: {
        level: 'GLOBAL',
        target: null,
        team: 'core',
        modules: [],
        files: [],
        symbols: [],
      },
      lifecycle_state: 'DURABLE',
      tags: ['auth', 'security'],
      supersedes: null,
      superseded_by: null,
      ratified_at_commit: 'abc1234',
      ratified_by: 'security-team',
      ratified_at: '2026-10-02T12:00:00Z',
    },
    {
      id: 'DEC-002',
      title: 'Payment Module Resiliency',
      statement: 'Exponential backoff retry policy.',
      reason: 'Mitigate transient network failures.',
      accepted_by: 'bob',
      accepted_at: '2026-10-05T12:00:00Z',
      scope: {
        level: 'MODULE',
        target: 'payment',
        team: 'checkout',
        modules: ['payment-service'],
        files: ['services/payment.ts'],
        symbols: [],
      },
      lifecycle_state: 'DURABLE',
      tags: ['resilience'],
      supersedes: null,
      superseded_by: null,
    },
    {
      id: 'DEC-003',
      title: 'Symbol Custom Rule',
      statement: 'Cache hash lookup table.',
      reason: 'O(1) memory lookup.',
      accepted_by: 'charlie',
      accepted_at: '2026-10-07T12:00:00Z',
      scope: {
        level: 'SYMBOL',
        target: 'cacheLookup',
        team: null,
        modules: [],
        files: [],
        symbols: ['cacheLookup'],
      },
      lifecycle_state: 'DURABLE',
      tags: [],
      supersedes: null,
      superseded_by: null,
    },
  ];

  beforeEach(() => {
    mockClient = {
      listMemories: async (_bin: string, _root: string) => sampleMemories,
    };
    provider = new MemoryTreeProvider(mockClient as unknown as EngramClient);
  });

  it('should return guidance item when binary is not resolved', async () => {
    const items = await provider.getChildren();
    assert.strictEqual(items.length, 1);
    assert.strictEqual(items[0].isStateItem, true);
    assert.match(items[0].label, /Engram binary not detected/i);
  });

  it('should return guidance item when workspace root is not open', async () => {
    const validRes: BinaryResolutionResult = {
      valid: true,
      path: '/path/to/engram',
      version: '1.4.0',
      source: 'path',
    };
    provider.setResolution(validRes); // workspaceRoot is undefined

    const items = await provider.getChildren();
    assert.strictEqual(items.length, 1);
    assert.strictEqual(items[0].isStateItem, true);
    assert.match(items[0].label, /Please open a workspace folder/i);
  });

  it('should return empty zero-state item when no memories exist', async () => {
    mockClient.listMemories = async () => [];
    const validRes: BinaryResolutionResult = {
      valid: true,
      path: '/path/to/engram',
      version: '1.4.0',
      source: 'path',
    };
    provider.setResolution(validRes, '/workspace/root');

    const items = await provider.getChildren();
    assert.strictEqual(items.length, 1);
    assert.strictEqual(items[0].isStateItem, true);
    assert.match(items[0].label, /No durable memories recorded/i);
  });

  it('should surface error item cleanly when listMemories fails', async () => {
    mockClient.listMemories = async () => {
      throw new Error('Database locked');
    };
    const validRes: BinaryResolutionResult = {
      valid: true,
      path: '/path/to/engram',
      version: '1.4.0',
      source: 'path',
    };
    provider.setResolution(validRes, '/workspace/root');

    const items = await provider.getChildren();
    assert.strictEqual(items.length, 1);
    assert.strictEqual(items[0].isStateItem, true);
    assert.match(items[0].label, /Error loading memories: Database locked/i);
  });

  it('should group memories into scope categories at the root level', async () => {
    const validRes: BinaryResolutionResult = {
      valid: true,
      path: '/path/to/engram',
      version: '1.4.0',
      source: 'path',
    };
    provider.setResolution(validRes, '/workspace/root');

    const rootItems = await provider.getChildren();
    assert.strictEqual(rootItems.length, 3, 'Should have GLOBAL, MODULE, and SYMBOL category items');

    const labels = rootItems.map((i) => i.label);
    assert.ok(labels.includes('GLOBAL Decisions (1)'));
    assert.ok(labels.includes('MODULE Decisions (1)'));
    assert.ok(labels.includes('SYMBOL Decisions (1)'));

    for (const item of rootItems) {
      assert.strictEqual(item.isCategory, true);
      assert.strictEqual(item.collapsibleState, vscode.TreeItemCollapsibleState.Expanded);
    }
  });

  it('should expand category items to reveal individual memories', async () => {
    const validRes: BinaryResolutionResult = {
      valid: true,
      path: '/path/to/engram',
      version: '1.4.0',
      source: 'path',
    };
    provider.setResolution(validRes, '/workspace/root');

    const rootItems = await provider.getChildren();
    const moduleCategory = rootItems.find((i) => i.scopeCategory === 'MODULE');
    assert.ok(moduleCategory, 'Module category must exist');

    const children = await provider.getChildren(moduleCategory);
    assert.strictEqual(children.length, 1);
    const memItem = children[0];
    assert.strictEqual(memItem.label, 'Payment Module Resiliency');
    assert.strictEqual(memItem.description, 'MODULE');
    assert.strictEqual(memItem.collapsibleState, vscode.TreeItemCollapsibleState.None);
    assert.strictEqual(memItem.command?.command, 'engram.showMemoryDetails');
    assert.strictEqual(memItem.command?.arguments?.[0].id, 'DEC-002');
  });

  it('should construct rich Markdown tooltips for memory items', async () => {
    const validRes: BinaryResolutionResult = {
      valid: true,
      path: '/path/to/engram',
      version: '1.4.0',
      source: 'path',
    };
    provider.setResolution(validRes, '/workspace/root');

    const rootItems = await provider.getChildren();
    const globalCategory = rootItems.find((i) => i.scopeCategory === 'GLOBAL');
    assert.ok(globalCategory);

    const children = await provider.getChildren(globalCategory);
    const globalMem = children[0];

    assert.ok(globalMem.tooltip instanceof vscode.MarkdownString);
    const tooltipText = (globalMem.tooltip as vscode.MarkdownString).value;
    assert.match(tooltipText, /### Global Auth Standard/);
    assert.match(tooltipText, /\*\*Decision ID:\*\* `DEC-001`/);
    assert.match(tooltipText, /\*\*Ratified At:\*\* `abc1234`/);
    assert.match(tooltipText, /\*\*Statement:\*\* Use JWT for all service auth\./);
  });

  it('should fire onDidChangeTreeData when refresh or setResolution is called', (done) => {
    let fired = 0;
    const disposable = provider.onDidChangeTreeData(() => {
      fired++;
      if (fired === 2) {
        disposable.dispose();
        done();
      }
    });

    provider.refresh();
    provider.setResolution({
      valid: true,
      path: '/bin/engram',
      version: '1.4.0',
      source: 'path',
    });
  });
});
