import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as vscode from 'vscode';
import { BinaryResolver } from '../src/binaryResolver';
import { McpClient } from '../src/mcpClient';
import { EngramLanguageModelToolProvider, GOVERNED_TOOLS } from '../src/languageModelTools';
import { SetupInstructionsManager, DEFAULT_COPILOT_INSTRUCTION } from '../src/setupInstructions';

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

describe('Automatic Engineering Intelligence Evaluation Suite', () => {
  const repoRoot = findRepoRoot(__dirname);
  const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
  const releaseBin = path.join(repoRoot, 'target', 'release', binName);
  const debugBin = fs.existsSync(releaseBin)
    ? releaseBin
    : path.join(repoRoot, 'target', 'debug', binName);

  // Angular test workspace if available
  const angularDir = 'C:\\Users\\Amal P S\\OneDrive\\Projects\\Test\\angular-ngrx-material-starter';
  const hasAngular = fs.existsSync(angularDir) && fs.existsSync(path.join(angularDir, '.engram'));

  let binaryResolver: BinaryResolver;
  let mcpClient: McpClient;
  let provider: EngramLanguageModelToolProvider;
  let mockContext: vscode.ExtensionContext;
  let instructionsManager: SetupInstructionsManager;
  let originalEngramPath: string | undefined;

  before(() => {
    originalEngramPath = process.env.ENGRAM_PATH;
    const preferredBin = fs.existsSync(releaseBin)
      ? releaseBin
      : (fs.existsSync(debugBin) ? debugBin : undefined);
    assert.ok(
      preferredBin,
      `Neither release binary (${releaseBin}) nor debug binary (${debugBin}) exists. Build engram before running automatic intelligence tests.`
    );
    process.env.ENGRAM_PATH = preferredBin;
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
    instructionsManager = new SetupInstructionsManager();

    const subscriptions: vscode.Disposable[] = [];
    mockContext = { subscriptions } as any;
  });

  afterEach(() => {
    for (const d of mockContext.subscriptions) {
      d.dispose();
    }
  });

  it('TEST ENVIRONMENT: resolves locally built release or debug binary via scoped ENGRAM_PATH', async () => {
    const resolution = await binaryResolver.resolve();
    assert.ok(resolution.valid, `Binary resolution failed: ${!resolution.valid ? resolution.error : ''}`);
    if (resolution.valid) {
      assert.strictEqual(resolution.source, 'env_engram_path', 'Must resolve via ENGRAM_PATH environment variable');
      const preferredBin = fs.existsSync(releaseBin) ? releaseBin : debugBin;
      assert.strictEqual(path.normalize(resolution.path), path.normalize(preferredBin), 'Must resolve local target binary');
    }
  });

  it('INTELLIGENCE GUIDANCE: instructions clearly direct Copilot on task scope and tool selection', () => {
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('Feature Implementation'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('engram_get_context'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('Modifying Shared Services'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('engram_get_impact'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('Architectural or High-Risk Modifications'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('engram_preflight'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('Post-Implementation Verification'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('engram_postflight'));
    assert.ok(DEFAULT_COPILOT_INSTRUCTION.includes('Trivial / Leaf Edits'));
  });

  it('TASK-AWARE CATALOG: modelDescriptions instruct Copilot when to call tools automatically', () => {
    const contextTool = GOVERNED_TOOLS.find((t) => t.name === 'engram_get_context');
    assert.ok(contextTool);
    assert.ok(contextTool.modelDescription.includes('Call automatically when starting implementation tasks'));

    const impactTool = GOVERNED_TOOLS.find((t) => t.name === 'engram_get_impact');
    assert.ok(impactTool);
    assert.ok(impactTool.modelDescription.includes('Call automatically before modifying shared services'));

    const preflightTool = GOVERNED_TOOLS.find((t) => t.name === 'engram_preflight');
    assert.ok(preflightTool);
    assert.ok(preflightTool.modelDescription.includes('before high-risk or architectural modifications'));

    const whyTool = GOVERNED_TOOLS.find((t) => t.name === 'engram_why');
    assert.ok(whyTool);
    assert.ok(whyTool.modelDescription.includes('Call automatically when investigating why code was structured'));
  });

  it('TASK 1 EVALUATION: Small Isolated Change requires zero/minimal blast radius', async function () {
    this.timeout(15000);
    const targetDir = hasAngular ? angularDir : repoRoot;
    provider.registerAll(mockContext, () => targetDir);

    // Leaf component in Angular project (about.component.ts)
    const result = await (vscode as any).lm.invokeTool('engram_get_impact', {
      input: { target: 'about.component.ts' },
    });
    assert.ok(result);
    const text = result.content[0].value;
    const parsed = JSON.parse(text);

    // Leaf component has minimal local reverse dependencies confined to its feature
    const dependents = parsed.reverse_dependencies || [];
    assert.ok(
      dependents.length <= 5,
      `Leaf component should have minimal reverse dependents (got ${dependents.length})`
    );
    if (hasAngular && dependents.length > 0) {
      assert.ok(
        dependents.every((d: any) => d.source.includes('about')),
        'All dependents must be confined to the local about feature folder'
      );
    }
  });

  it('TASK 2 EVALUATION: Feature Implementation retrieves architectural constraints and scoped context', async function () {
    this.timeout(20000);
    const targetDir = hasAngular ? angularDir : repoRoot;
    provider.registerAll(mockContext, () => targetDir);

    const start = Date.now();
    const result = await (vscode as any).lm.invokeTool('engram_get_context', {
      input: { task: 'Add refresh-token support to the authentication flow', token_budget: 1500 },
    });
    const latency = Date.now() - start;

    assert.ok(result);
    const text = result.content[0].value;
    assert.ok(text.length > 50);

    // If Angular workspace, verify rule extraction
    if (hasAngular) {
      assert.ok(
        text.includes('No direct fetch in components') || text.includes('confidence') || text.includes('files'),
        'Context should contain architecture rules or file recommendations'
      );
    }
  });

  it('TASK 3 EVALUATION: Shared-Service Change identifies all reverse dependents', async function () {
    this.timeout(15000);
    if (!hasAngular) {
      this.skip();
      return;
    }

    provider.registerAll(mockContext, () => angularDir);

    const result = await (vscode as any).lm.invokeTool('engram_get_impact', {
      input: { target: 'local-storage.service.ts' },
    });
    assert.ok(result);
    const text = result.content[0].value;
    const parsed = JSON.parse(text);

    assert.ok(Array.isArray(parsed.reverse_dependencies));
    assert.ok(parsed.reverse_dependencies.length >= 5, 'Must detect at least 5 dependents for LocalStorageService');

    const sources = parsed.reverse_dependencies.map((d: any) => d.source);
    assert.ok(sources.some((s: string) => s.includes('auth.effects')));
    assert.ok(sources.some((s: string) => s.includes('settings.effects')));
    assert.ok(sources.some((s: string) => s.includes('core.module')));
  });

  it('TASK 4 EVALUATION: Cross-Module Refactor identifies multi-module blast radius and locks contract', async function () {
    this.timeout(20000);
    if (!hasAngular) {
      this.skip();
      return;
    }

    provider.registerAll(mockContext, () => angularDir);

    // 1. Impact check
    const impactRes = await (vscode as any).lm.invokeTool('engram_get_impact', {
      input: { target: 'src/app/core/core.module.ts' },
    });
    assert.ok(impactRes);
    const impactParsed = JSON.parse(impactRes.content[0].value);
    assert.strictEqual(impactParsed.reverse_dependencies.length, 31);

    // 2. Preflight contract lock
    const preflightRes = await (vscode as any).lm.invokeTool('engram_preflight', {
      input: { task: 'Refactor the CoreModule auth interceptor and state routing structure' },
    });
    assert.ok(preflightRes);
    const preflightText = preflightRes.content[0].value;
    assert.ok(
      preflightText.includes('READY_FOR_IMPLEMENTATION') || preflightText.includes('contract_id'),
      'Preflight should lock Change Contract'
    );
  });

  it('TASK 5 EVALUATION: Multi-File Bug Fix retrieves state flow context and why rationale', async function () {
    this.timeout(20000);
    if (!hasAngular) {
      this.skip();
      return;
    }

    provider.registerAll(mockContext, () => angularDir);

    // Retrieve context for state bug
    const ctxRes = await (vscode as any).lm.invokeTool('engram_get_context', {
      input: {
        task: 'Fix the stock market effect selector mismatch where stock data is not reflected in component',
        token_budget: 2000,
      },
    });
    assert.ok(ctxRes);
    const ctxText = ctxRes.content[0].value;
    assert.ok(ctxText.length > 50);

    // Retrieve why rationale
    const whyRes = await (vscode as any).lm.invokeTool('engram_why', {
      input: { target: 'stock-market.effects.ts' },
    });
    assert.ok(whyRes);
    const whyText = whyRes.content[0].value;
    assert.ok(whyText.includes('target_symbol') || whyText.includes('rationale'));
  });
});
