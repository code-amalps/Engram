import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { McpRegistrationManager } from '../src/mcpRegistration';

describe('McpRegistrationManager Tests', () => {
  let manager: McpRegistrationManager;
  let tempDir: string;

  beforeEach(() => {
    manager = new McpRegistrationManager();
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram-reg-test-'));
  });

  afterEach(() => {
    try {
      fs.rmSync(tempDir, { recursive: true, force: true });
    } catch {
      // ignore
    }
  });

  it('should report NOT_CONFIGURED when .vscode/mcp.json does not exist', () => {
    const result = manager.inspect(tempDir);
    assert.strictEqual(result.status, 'NOT_CONFIGURED');
  });

  it('should register engram server into fresh workspace with native VS Code schema', () => {
    const regResult = manager.register(tempDir, 'engram');
    assert.strictEqual(regResult.success, true);
    assert.strictEqual(regResult.changed, true);

    const configPath = manager.getConfigPath(tempDir);
    assert.ok(fs.existsSync(configPath));

    const content = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    assert.ok(content.servers?.engram, 'Must have servers.engram');
    assert.strictEqual(content.servers.engram.type, 'stdio');
    assert.strictEqual(content.servers.engram.command, 'engram');
    assert.deepStrictEqual(content.servers.engram.args, ['-C', '${workspaceFolder}', 'mcp']);

    const inspectResult = manager.inspect(tempDir);
    assert.strictEqual(inspectResult.status, 'CONFIGURED_HEALTHY');
  });

  it('STRICT IDEMPOTENCY: should not alter file or report change on repeated register', () => {
    const res1 = manager.register(tempDir, 'engram');
    assert.strictEqual(res1.changed, true);

    const configPath = manager.getConfigPath(tempDir);
    const contentBefore = fs.readFileSync(configPath, 'utf8');

    const res2 = manager.register(tempDir, 'engram');
    assert.strictEqual(res2.success, true);
    assert.strictEqual(res2.changed, false);

    const contentAfter = fs.readFileSync(configPath, 'utf8');
    assert.strictEqual(contentAfter, contentBefore, 'File bytes must remain identical');
  });

  it('PRESERVATION: should preserve unrelated third-party servers and custom configuration', () => {
    const configPath = manager.getConfigPath(tempDir);
    fs.mkdirSync(path.dirname(configPath), { recursive: true });

    const initial = {
      userCustomKey: 'preserved_value',
      servers: {
        github: {
          type: 'stdio',
          command: 'gh',
          args: ['copilot', 'mcp'],
        },
      },
    };
    fs.writeFileSync(configPath, JSON.stringify(initial, null, 2), 'utf8');

    const regResult = manager.register(tempDir, 'engram');
    assert.strictEqual(regResult.success, true);

    const updated = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    assert.strictEqual(updated.userCustomKey, 'preserved_value');
    assert.ok(updated.servers.github, 'Must preserve third-party github server');
    assert.ok(updated.servers.engram, 'Must add engram server');
  });

  it('JSONC SUPPORT: should safely parse configuration containing line and block comments', () => {
    const configPath = manager.getConfigPath(tempDir);
    fs.mkdirSync(path.dirname(configPath), { recursive: true });

    const jsonWithComments = `// VS Code MCP configuration
{
  /* Active custom servers */
  "servers": {
    "existing": {
      "type": "stdio",
      "command": "node"
    }
  }
}
`;
    fs.writeFileSync(configPath, jsonWithComments, 'utf8');

    const regResult = manager.register(tempDir, 'engram');
    assert.strictEqual(regResult.success, true);

    const inspectResult = manager.inspect(tempDir);
    assert.strictEqual(inspectResult.status, 'CONFIGURED_HEALTHY');
  });

  it('CONFLICT DETECTION: should detect conflict if engram command or args are altered', () => {
    const configPath = manager.getConfigPath(tempDir);
    fs.mkdirSync(path.dirname(configPath), { recursive: true });

    const conflictConfig = {
      servers: {
        engram: {
          type: 'stdio',
          command: '/wrong/path/to/binary',
          args: ['--invalid-flag'],
        },
      },
    };
    fs.writeFileSync(configPath, JSON.stringify(conflictConfig), 'utf8');

    const inspectResult = manager.inspect(tempDir, 'engram');
    assert.strictEqual(inspectResult.status, 'CONFLICT');
    assert.ok(inspectResult.details?.includes('does not match expected'));

    // Repair with force
    const fixResult = manager.register(tempDir, 'engram', true);
    assert.strictEqual(fixResult.success, true);
    assert.strictEqual(fixResult.changed, true);

    const afterFix = manager.inspect(tempDir, 'engram');
    assert.strictEqual(afterFix.status, 'CONFIGURED_HEALTHY');
  });

  it('SURGICAL REMOVE: should remove only engram server and keep other servers intact', () => {
    const configPath = manager.getConfigPath(tempDir);
    fs.mkdirSync(path.dirname(configPath), { recursive: true });

    const initial = {
      servers: {
        engram: {
          type: 'stdio',
          command: 'engram',
          args: ['-C', '${workspaceFolder}', 'mcp'],
        },
        other: {
          command: 'other-tool',
        },
      },
    };
    fs.writeFileSync(configPath, JSON.stringify(initial), 'utf8');

    const remResult = manager.remove(tempDir);
    assert.strictEqual(remResult.success, true);
    assert.strictEqual(remResult.removed, true);

    const content = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    assert.strictEqual(content.servers.engram, undefined);
    assert.ok(content.servers.other);
  });

  it('CROSS-PLATFORM & SPACES: should function reliably in paths containing spaces', () => {
    const spacedDir = path.join(tempDir, 'Project Space Name with Special Chars');
    fs.mkdirSync(spacedDir, { recursive: true });

    const regResult = manager.register(spacedDir, 'engram');
    assert.strictEqual(regResult.success, true);

    const inspectResult = manager.inspect(spacedDir);
    assert.strictEqual(inspectResult.status, 'CONFIGURED_HEALTHY');
  });

  it('DUAL REGISTRATION: should write root .mcp.json alongside .vscode/mcp.json for agent harness discovery', () => {
    const regResult = manager.register(tempDir, 'engram');
    assert.strictEqual(regResult.success, true);

    // Verify .vscode/mcp.json
    const vsCodeConfig = JSON.parse(fs.readFileSync(manager.getConfigPath(tempDir), 'utf8'));
    assert.ok(vsCodeConfig.servers?.engram, 'Must have servers.engram in .vscode/mcp.json');
    assert.strictEqual(vsCodeConfig.servers.engram.type, 'stdio');

    // Verify root .mcp.json
    const rootConfigPath = manager.getRootConfigPath(tempDir);
    assert.ok(fs.existsSync(rootConfigPath), 'Root .mcp.json must exist');
    const rootConfig = JSON.parse(fs.readFileSync(rootConfigPath, 'utf8'));
    assert.ok(rootConfig.mcpServers?.engram, 'Must have mcpServers.engram in root .mcp.json');
    assert.strictEqual(rootConfig.mcpServers.engram.command, 'engram');
    assert.deepStrictEqual(rootConfig.mcpServers.engram.args, ['-C', '${workspaceFolder}', 'mcp']);

    // Verify inspect recognizes the setup
    const inspectResult = manager.inspect(tempDir);
    assert.strictEqual(inspectResult.status, 'CONFIGURED_HEALTHY');

    // Verify remove cleans up both
    const remResult = manager.remove(tempDir);
    assert.strictEqual(remResult.success, true);
    assert.strictEqual(remResult.removed, true);

    const vsCodeAfter = JSON.parse(fs.readFileSync(manager.getConfigPath(tempDir), 'utf8'));
    assert.strictEqual(vsCodeAfter.servers?.engram, undefined);

    const rootAfter = JSON.parse(fs.readFileSync(rootConfigPath, 'utf8'));
    assert.strictEqual(rootAfter.mcpServers?.engram, undefined);
  });

  it('FALLBACK INSPECTION: should inspect root .mcp.json when .vscode/mcp.json is absent', () => {
    const rootConfigPath = manager.getRootConfigPath(tempDir);
    const rootConfig = {
      mcpServers: {
        engram: {
          command: 'engram',
          args: ['-C', '${workspaceFolder}', 'mcp'],
        },
      },
    };
    fs.writeFileSync(rootConfigPath, JSON.stringify(rootConfig), 'utf8');

    // Confirm .vscode/mcp.json does NOT exist
    assert.strictEqual(fs.existsSync(manager.getConfigPath(tempDir)), false);

    const inspectResult = manager.inspect(tempDir);
    assert.strictEqual(inspectResult.status, 'CONFIGURED_HEALTHY');
    assert.strictEqual(inspectResult.path, rootConfigPath);
  });
});
