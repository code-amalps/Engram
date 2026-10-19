import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { SetupManager } from '../src/setupManager';
import { BinaryResolver } from '../src/binaryResolver';
import { WorkspaceDetector } from '../src/workspaceDetector';
import { McpRegistrationManager } from '../src/mcpRegistration';
import { McpClient } from '../src/mcpClient';
import { SetupInstructionsManager, START_MARKER, END_MARKER } from '../src/setupInstructions';

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

describe('SetupManager Tests', () => {
  let setupManager: SetupManager;
  let binaryResolver: BinaryResolver;
  let workspaceDetector: WorkspaceDetector;
  let mcpRegistration: McpRegistrationManager;
  let mcpClient: McpClient;
  let instructionsManager: SetupInstructionsManager;
  let tempDir: string;

  const repoRoot = findRepoRoot(__dirname);
  const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
  const releaseBin = path.join(repoRoot, 'target', 'release', binName);
  const debugBin = fs.existsSync(releaseBin)
    ? releaseBin
    : path.join(repoRoot, 'target', 'debug', binName);

  beforeEach(() => {
    binaryResolver = new BinaryResolver();
    workspaceDetector = new WorkspaceDetector();
    mcpRegistration = new McpRegistrationManager();
    mcpClient = new McpClient();
    instructionsManager = new SetupInstructionsManager();
    setupManager = new SetupManager(
      binaryResolver,
      workspaceDetector,
      mcpRegistration,
      mcpClient,
      instructionsManager
    );
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram-setup-mgr-'));
  });

  afterEach(() => {
    try {
      fs.rmSync(tempDir, { recursive: true, force: true });
    } catch {
      // ignore
    }
  });

  it('STAGE CHECK: should report SETUP_REQUIRED on unconfigured workspace', async () => {
    const report = await setupManager.checkSetup(tempDir, { runHandshake: false });
    assert.strictEqual(report.registration.status, 'NOT_CONFIGURED');
    assert.strictEqual(report.recommendedAction, 'register_mcp');
  });

  it('1-CLICK REPAIR & VERIFICATION: should register MCP and verify setup end-to-end', async function () {
    this.timeout(20000);

    if (!fs.existsSync(debugBin)) {
      this.skip();
      return;
    }

    // Initialize mock Angular project
    fs.writeFileSync(path.join(tempDir, 'angular.json'), '{}', 'utf8');

    // Run 1-Click Fix
    const fixResult = await setupManager.fixSetup(tempDir, {
      explicitBinary: debugBin,
      force: true,
    });

    assert.strictEqual(fixResult.success, true, `Fix setup failed: ${fixResult.message}`);
    assert.strictEqual(fixResult.report.state, 'READY');
    assert.ok(fixResult.report.health?.success);
    assert.ok(fixResult.report.health.toolCount >= 15);
    assert.ok(fixResult.report.workspace?.detectedProjects.includes('angular'));

    // Check .vscode/mcp.json exists and uses native VS Code schema
    const mcpFile = path.join(tempDir, '.vscode', 'mcp.json');
    assert.ok(fs.existsSync(mcpFile));
    const content = JSON.parse(fs.readFileSync(mcpFile, 'utf8'));
    assert.ok(content.servers?.engram);
    assert.strictEqual(content.servers.engram.type, 'stdio');
    assert.deepStrictEqual(content.servers.engram.args, ['-C', '${workspaceFolder}', 'mcp']);
  });

  it('INSTRUCTION SYNC: should configure and strip Copilot instructions cleanly', () => {
    const res = instructionsManager.syncInstructions(tempDir, false);
    assert.strictEqual(res.success, true);
    assert.strictEqual(res.changed, true);

    const instFile = instructionsManager.getInstructionsPath(tempDir);
    assert.ok(fs.existsSync(instFile));
    const content = fs.readFileSync(instFile, 'utf8');
    assert.ok(content.includes(START_MARKER));
    assert.ok(content.includes(END_MARKER));
    assert.ok(content.includes('engram_get_context'));

    // Repeated sync is idempotent
    const res2 = instructionsManager.syncInstructions(tempDir, false);
    assert.strictEqual(res2.changed, false);

    // Removal strips block
    const remRes = instructionsManager.syncInstructions(tempDir, true);
    assert.strictEqual(remRes.success, true);
    assert.strictEqual(remRes.changed, true);
    assert.ok(!fs.existsSync(instFile), 'Empty file should be removed');
  });

  it('DISPOSABLE FIXTURES: should detect and configure .NET and Python fixtures with spaces in path', async function () {
    this.timeout(20000);

    if (!fs.existsSync(debugBin)) {
      this.skip();
      return;
    }

    const spacedDir = path.join(tempDir, 'Enterprise Core Service');
    fs.mkdirSync(spacedDir, { recursive: true });

    // .NET fixture
    fs.writeFileSync(path.join(spacedDir, 'EnterpriseCore.csproj'), '<Project></Project>', 'utf8');
    // Python helper script
    fs.writeFileSync(path.join(spacedDir, 'pyproject.toml'), '[project]', 'utf8');

    const wsInfo = workspaceDetector.detect(spacedDir);
    assert.ok(wsInfo.detectedProjects.includes('dotnet'));
    assert.ok(wsInfo.detectedProjects.includes('python'));

    const fix = await setupManager.fixSetup(spacedDir, {
      explicitBinary: debugBin,
      force: true,
    });
    assert.strictEqual(fix.success, true);
    assert.strictEqual(fix.report.state, 'READY');
  });
});
