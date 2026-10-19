import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as vscode from 'vscode';
import { BinaryResolver } from '../src/binaryResolver';
import { McpClient } from '../src/mcpClient';
import { EngramLanguageModelToolProvider, GOVERNED_TOOLS } from '../src/languageModelTools';

function findRepoRoot(startDir: string): string {
  let cur = startDir;
  while (cur !== path.dirname(cur)) {
    if (fs.existsSync(path.join(cur, 'Cargo.toml'))) {
      return cur;
    }
    cur = path.dirname(cur);
  }
  return startDir;
}

describe('LanguageModelTools Tests', () => {
  const repoRoot = findRepoRoot(__dirname);
  const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
  const releaseBin = path.join(repoRoot, 'target', 'release', binName);
  const debugBin = fs.existsSync(releaseBin)
    ? releaseBin
    : path.join(repoRoot, 'target', 'debug', binName);

  let binaryResolver: BinaryResolver;
  let mcpClient: McpClient;
  let provider: EngramLanguageModelToolProvider;
  let mockContext: vscode.ExtensionContext;
  let originalEngramPath: string | undefined;

  before(() => {
    originalEngramPath = process.env.ENGRAM_PATH;
    const preferredBin = fs.existsSync(releaseBin)
      ? releaseBin
      : (fs.existsSync(debugBin) ? debugBin : undefined);
    if (preferredBin) {
      process.env.ENGRAM_PATH = preferredBin;
    }
  });

  after(() => {
    if (originalEngramPath !== undefined) {
      process.env.ENGRAM_PATH = originalEngramPath;
    } else {
      delete process.env.ENGRAM_PATH;
    }
  });

  beforeEach(() => {
    const preferredBin = fs.existsSync(releaseBin)
      ? releaseBin
      : (fs.existsSync(debugBin) ? debugBin : undefined);
    if (preferredBin) {
      process.env.ENGRAM_PATH = preferredBin;
    }

    binaryResolver = new BinaryResolver();
    mcpClient = new McpClient();
    provider = new EngramLanguageModelToolProvider(binaryResolver, mcpClient);

    const subscriptions: vscode.Disposable[] = [];
    mockContext = {
      subscriptions,
    } as any;
  });

  afterEach(() => {
    for (const d of mockContext.subscriptions) {
      d.dispose();
    }
  });

  it('CATALOG: should declare exactly 15 governed tools with valid schemas and descriptions', () => {
    assert.strictEqual(GOVERNED_TOOLS.length, 15);
    const expected = [
      'engram_check_constitution',
      'engram_doctor',
      'engram_drift',
      'engram_get_cochanges',
      'engram_get_context',
      'engram_get_ecosystem',
      'engram_get_impact',
      'engram_get_memory',
      'engram_inspect_symbol',
      'engram_postflight',
      'engram_preflight',
      'engram_ratify_decision',
      'engram_record_decision',
      'engram_recover',
      'engram_why',
    ];
    for (const name of expected) {
      const found = GOVERNED_TOOLS.find((t) => t.name === name);
      assert.ok(found, `Tool ${name} must be declared in GOVERNED_TOOLS`);
      assert.ok(found.modelDescription && found.modelDescription.length > 20);
      assert.ok(found.inputSchema && found.inputSchema.type === 'object');
      assert.strictEqual(found.canBeReferencedInPrompt, true);
      assert.ok(found.toolReferenceName && found.toolReferenceName.length > 0);
      assert.ok(found.userDescription && found.userDescription.length > 0);
      assert.ok(found.icon && found.icon.startsWith('$('));
    }
  });

  it('REGISTRATION: should register all 15 tools into vscode.lm', () => {
    const disposables = provider.registerAll(mockContext, () => repoRoot);
    assert.strictEqual(disposables.length, 15);
    assert.strictEqual(mockContext.subscriptions.length, 15);

    // Verify tools exist in vscode.lm
    const registered = (vscode as any).lm._registeredTools as Map<string, any>;
    assert.ok(registered.has('engram_get_context'));
    assert.ok(registered.has('engram_doctor'));
    assert.ok(registered.has('engram_preflight'));
    assert.ok(registered.has('engram_postflight'));
    assert.ok(registered.has('engram_get_impact'));
  });

  it('ERROR HANDLING: should return clean error when no workspace is open', async () => {
    provider.registerAll(mockContext, () => undefined);

    const result = await (vscode as any).lm.invokeTool('engram_doctor', { input: {} });
    assert.ok(result);
    assert.ok(result.content[0].value.includes('No active workspace folder open'));
  });

  it('ERROR HANDLING: should return clean error when binary cannot be resolved', async () => {
    // Resolver pointing to nonexistent folder
    const fakeResolver = {
      resolve: async () => ({
        valid: false as const,
        error: 'Binary not found in PATH',
        isExplicitConfigError: false,
      }),
    } as any;

    const testProvider = new EngramLanguageModelToolProvider(fakeResolver, mcpClient);
    testProvider.registerAll(mockContext, () => repoRoot);

    const result = await (vscode as any).lm.invokeTool('engram_doctor', { input: {} });
    assert.ok(result);
    assert.ok(result.content[0].value.includes('Engram binary not found'));
  });

  it('REAL MCP INVOCATION: should invoke engram_doctor via vscode.lm and return valid LanguageModelToolResult', async function () {
    this.timeout(15000);

    const preferredBin = fs.existsSync(releaseBin)
      ? releaseBin
      : (fs.existsSync(debugBin) ? debugBin : undefined);
    assert.ok(
      preferredBin,
      `Neither release binary (${releaseBin}) nor debug binary (${debugBin}) exists. Build engram before running real MCP invocation tests.`
    );

    provider.registerAll(mockContext, () => repoRoot);

    const result = await (vscode as any).lm.invokeTool('engram_doctor', { input: {} });
    assert.ok(result, 'Result must exist');
    assert.ok(Array.isArray(result.content), 'Result must have content array');
    const text = result.content[0].value;
    assert.ok(typeof text === 'string', 'Value must be a string');
    assert.ok(
      text.includes('protocol_version') || text.includes('live_count') || text.includes('items'),
      `Result text should contain doctor metrics: ${text}`
    );
  });

  it('REAL MCP INVOCATION: should invoke engram_get_context via vscode.lm', async function () {
    this.timeout(15000);

    const preferredBin = fs.existsSync(releaseBin)
      ? releaseBin
      : (fs.existsSync(debugBin) ? debugBin : undefined);
    assert.ok(
      preferredBin,
      `Neither release binary (${releaseBin}) nor debug binary (${debugBin}) exists. Build engram before running real MCP invocation tests.`
    );

    provider.registerAll(mockContext, () => repoRoot);

    const result = await (vscode as any).lm.invokeTool('engram_get_context', {
      input: { task: 'Verify tool discovery and invocation' },
    });
    assert.ok(result, 'Result must exist');
    const text = result.content[0].value;
    assert.ok(typeof text === 'string');
    assert.ok(
      text.includes('task') || text.includes('context') || text.includes('files'),
      `Result text should contain compiled context: ${text}`
    );
  });
});
