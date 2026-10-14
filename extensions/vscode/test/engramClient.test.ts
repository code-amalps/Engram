import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as vscode from 'vscode';
import { EngramClient } from '../src/engramClient';
import { EngramOutputChannel } from '../src/outputChannel';

// Mock output channel for headless test execution
class MockOutputChannel extends EngramOutputChannel {
  public logs: string[] = [];
  constructor() {
    super();
  }
  public override log(message: string, level: 'INFO' | 'WARN' | 'ERROR' = 'INFO'): void {
    this.logs.push(`[${level}] ${message}`);
  }
  public override show(): void {}
  public override clear(): void {
    this.logs = [];
  }
  public override dispose(): void {}
}

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

describe('EngramClient Tests', () => {
  let mockChannel: MockOutputChannel = new MockOutputChannel();
  let client: EngramClient = new EngramClient(mockChannel);
  const repoRoot = findRepoRoot(__dirname);
  const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
  const debugBin = path.join(repoRoot, 'target', 'debug', binName);

  beforeEach(() => {
    mockChannel = new MockOutputChannel();
    client = new EngramClient(mockChannel);
  });

  describe('Schema Validation', () => {
    it('should validate complete valid doctor JSON', () => {
      const sample = {
        protocol_version: '1.1',
        total_memories: 1,
        live_count: 1,
        dormant_count: 0,
        stale_count: 0,
        superseded_count: 0,
        archival_candidate_count: 0,
        items: [
          {
            memory_id: 'mem_123',
            title: 'Test Architecture',
            scope_level: 'MODULE',
            scope_target: null,
            liveness: 'LIVE',
            diagnostic_message: 'Healthy',
            suggested_actions: ['None'],
          },
        ],
        event_chain_status: 'EVENT_CHAIN_VALID',
        event_chain_details: 'Verified',
      };

      const validated = client.validateDoctorReport(sample);
      assert.strictEqual(validated.total_memories, 1);
      assert.strictEqual(validated.live_count, 1);
      assert.strictEqual(validated.items.length, 1);
      assert.strictEqual(validated.items[0].memory_id, 'mem_123');
    });

    it('SAFEGUARD 4: should validate uninitialized workspace zero-state JSON as success', () => {
      const zeroState = {
        protocol_version: '1.1',
        total_memories: 0,
        live_count: 0,
        dormant_count: 0,
        stale_count: 0,
        superseded_count: 0,
        archival_candidate_count: 0,
        items: [],
        event_chain_status: 'EVENT_CHAIN_VALID',
        event_chain_details: '0 events recorded (empty chain pristine)',
      };

      const validated = client.validateDoctorReport(zeroState);
      assert.strictEqual(validated.total_memories, 0);
      assert.strictEqual(validated.live_count, 0);
      assert.strictEqual(validated.items.length, 0);
      assert.strictEqual(validated.event_chain_status, 'EVENT_CHAIN_VALID');
    });

    it('should throw when schema is invalid or fields are missing', () => {
      assert.throws(() => client.validateDoctorReport(null), /Doctor response is not a JSON object/);
      assert.throws(() => client.validateDoctorReport({}), /Missing or invalid "protocol_version"/);
      assert.throws(
        () => client.validateDoctorReport({ protocol_version: '1.1' }),
        /Missing or invalid "total_memories"/
      );
      assert.throws(
        () => client.validateDoctorReport({ protocol_version: '1.1', total_memories: 0, live_count: 0, items: 'not-array' }),
        /Missing or invalid "items" array/
      );
    });
  });

  describe('Secret Sanitization', () => {
    it('should redact sensitive tokens and credentials from logs', () => {
      const channel = new EngramOutputChannel();
      const raw = 'Error connecting with sk-1234567890abcdef1234567890 and ghp_123456789012345678901234567890123456 and password=supersecret';
      const sanitized = channel.sanitize(raw);
      assert.ok(!sanitized.includes('sk-1234567890abcdef1234567890'));
      assert.ok(!sanitized.includes('ghp_123456789012345678901234567890123456'));
      assert.ok(!sanitized.includes('supersecret'));
      assert.ok(sanitized.includes('[REDACTED_API_KEY]'));
      assert.ok(sanitized.includes('[REDACTED_GITHUB_TOKEN]'));
      assert.ok(sanitized.includes('password=[REDACTED]'));
    });
  });

  describe('Real CLI Invocations (when binary is built)', () => {
    it('should execute doctor on real Engram repository workspace', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      const report = await client.doctor(debugBin, repoRoot);
      assert.strictEqual(report.protocol_version, '1.1');
      assert.ok(report.total_memories >= 0);
      assert.strictEqual(report.event_chain_status, 'EVENT_CHAIN_VALID');
    });

    it('SAFEGUARD 4: should succeed on empty uninitialized workspace directory', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram_client_test_'));
      try {
        const report = await client.doctor(debugBin, tempDir);
        assert.strictEqual(report.total_memories, 0);
        assert.strictEqual(report.live_count, 0);
        assert.strictEqual(report.items.length, 0);
        assert.strictEqual(report.event_chain_status, 'EVENT_CHAIN_VALID');
      } finally {
        fs.rmSync(tempDir, { recursive: true, force: true });
      }
    });

    it('SAFEGUARD 4: should surface corrupted workspace error clearly', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram_corrupt_test_'));
      const engramDir = path.join(tempDir, '.engram');
      fs.mkdirSync(engramDir, { recursive: true });
      fs.writeFileSync(path.join(engramDir, 'memory.yaml'), 'invalid: [unclosed yaml');

      try {
        await assert.rejects(
          async () => {
            await client.doctor(debugBin, tempDir);
          },
          (err: any) => {
            assert.match(err.message, /Engram Doctor failed/i);
            return true;
          }
        );
      } finally {
        fs.rmSync(tempDir, { recursive: true, force: true });
      }
    });

    it('should reject concurrent doctor invocations', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      // Start first doctor call
      const p1 = client.doctor(debugBin, repoRoot);
      // Attempt concurrent call immediately
      await assert.rejects(
        async () => {
          await client.doctor(debugBin, repoRoot);
        },
        /A health check is already running/
      );

      await p1;
    });

    it('SECURITY: should safely handle arguments with spaces and shell metacharacters without shell execution', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      // Create a real directory whose path contains spaces and shell injection metacharacters
      const untrustedPath = fs.mkdtempSync(path.join(os.tmpdir(), 'engram test space; echo injected && dir '));
      try {
        // Because shell: false is enforced with an argument vector, the path is passed literally to the binary.
        // It treats it cleanly as an uninitialized workspace without executing shell commands or failing on spaces.
        const report = await client.doctor(debugBin, untrustedPath);
        assert.strictEqual(report.total_memories, 0);
        assert.strictEqual(report.event_chain_status, 'EVENT_CHAIN_VALID');
      } finally {
        fs.rmSync(untrustedPath, { recursive: true, force: true });
      }
    });

    it('RELIABILITY: should handle cancellation gracefully', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      const cts = new vscode.CancellationTokenSource();
      const promise = client.doctor(debugBin, repoRoot, 30000, cts.token);

      // Cancel immediately
      cts.cancel();

      await assert.rejects(
        promise,
        /Operation cancelled by user/
      );
    });

    it('RELIABILITY: should reject immediately if token is already cancelled before execution', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      const cts = new vscode.CancellationTokenSource();
      cts.cancel(); // Pre-cancel before calling doctor

      await assert.rejects(
        client.doctor(debugBin, repoRoot, 30000, cts.token),
        /Operation cancelled by user/
      );
    });

    it('RELIABILITY: should handle timeouts cleanly', async function () {
      if (!fs.existsSync(debugBin)) {
        this.skip();
        return;
      }

      // Set ultra-low timeout of 1ms to trigger timeout handler
      await assert.rejects(
        async () => {
          await client.doctor(debugBin, repoRoot, 1);
        },
        /Command timed out after 1ms/
      );
    });
  });
});
