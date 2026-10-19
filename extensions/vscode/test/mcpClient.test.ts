import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { McpClient } from '../src/mcpClient';

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

describe('McpClient Tests', () => {
  let client: McpClient;
  const repoRoot = findRepoRoot(__dirname);
  const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
  const releaseBin = path.join(repoRoot, 'target', 'release', binName);
  const debugBin = fs.existsSync(releaseBin)
    ? releaseBin
    : path.join(repoRoot, 'target', 'debug', binName);

  beforeEach(() => {
    client = new McpClient();
  });

  it('should fail cleanly when binary does not exist', async () => {
    const bogus = path.join(os.tmpdir(), 'nonexistent_engram_binary_xyz123');
    const result = await client.checkHealth(bogus, repoRoot, 2000);
    assert.strictEqual(result.success, false);
    assert.ok(result.error);
  });

  it('REAL MCP PROTOCOL: should handshake, negotiate protocol, and discover governed tools', async function () {
    this.timeout(15000);

    if (!fs.existsSync(debugBin)) {
      this.skip();
      return;
    }

    const result = await client.checkHealth(debugBin, repoRoot, 8000);
    assert.strictEqual(
      result.success,
      true,
      `MCP handshake must succeed: ${result.error}`
    );
    assert.strictEqual(result.serverName, 'engram');
    assert.strictEqual(result.protocolVersion, '2024-11-05');
    assert.ok(result.toolCount >= 15, `Must discover at least 15 tools (got ${result.toolCount})`);

    // Verify all 15 governed tools are discovered
    const expectedTools = [
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

    for (const tool of expectedTools) {
      assert.ok(
        result.tools.includes(tool),
        `MCP tools must include '${tool}'`
      );
    }
  });

  it('REAL MCP RPC CALL: should execute read-only tool through stdio connection', async function () {
    this.timeout(15000);

    if (!fs.existsSync(debugBin)) {
      this.skip();
      return;
    }

    const callResult = await client.callTool(debugBin, repoRoot, 'engram_doctor', {}, 8000);
    assert.strictEqual(
      callResult.success,
      true,
      `Tool call engram_doctor must succeed: ${callResult.error}`
    );
    assert.ok(callResult.result, 'Must return result payload');
  });
});
