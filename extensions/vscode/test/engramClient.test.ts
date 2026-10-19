import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as cp from 'child_process';
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
  const releaseBin = path.join(repoRoot, 'target', 'release', binName);
  const debugBin = fs.existsSync(releaseBin)
    ? releaseBin
    : path.join(repoRoot, 'target', 'debug', binName);

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

    describe('validateMemoryList', () => {
      it('should validate and parse memory list response correctly', () => {
        const raw = {
          version: 1,
          memories: [
            {
              id: 'DEC-001',
              title: 'API Design Standard',
              statement: 'Use JSON:API specifications.',
              reason: 'Consistent error responses across endpoints.',
              accepted_by: 'lead',
              accepted_at: '2026-10-01T10:00:00Z',
              scope: {
                level: 'GLOBAL',
                target: null,
                team: 'platform',
                modules: ['api'],
                files: [],
                symbols: [],
              },
              lifecycle_state: 'DURABLE',
              tags: ['api', 'standards'],
            },
          ],
        };

        const res = client.validateMemoryList(raw);
        assert.strictEqual(res.length, 1);
        assert.strictEqual(res[0].id, 'DEC-001');
        assert.strictEqual(res[0].title, 'API Design Standard');
        assert.strictEqual(res[0].scope.level, 'GLOBAL');
        assert.strictEqual(res[0].scope.modules[0], 'api');
      });

      it('should accept empty memories array', () => {
        const res = client.validateMemoryList({ version: 1, memories: [] });
        assert.deepStrictEqual(res, []);
      });

      it('should throw when memory list is invalid', () => {
        assert.throws(() => client.validateMemoryList(null), /not a valid JSON object/);
        assert.throws(() => client.validateMemoryList({}), /does not contain a "memories" array/);
        assert.throws(
          () => client.validateMemoryList({ memories: ['not-an-object'] }),
          /is not an object/
        );
      });
    });

    describe('validateContextReport', () => {
      it('should validate and parse context report response correctly', () => {
        const raw = {
          task: 'Fix race condition in session manager',
          project: 'AMstraLift',
          revision: 'abc987',
          indexed_files: 50,
          token_budget: 2000,
          estimated_tokens: 450,
          confidence: 'HIGH',
          constraints: ['No breaking changes'],
          relevant_memories: [],
          cochange_alerts: [],
          ecosystem_guard: 'Rust 2021',
          gaps: [],
          files: [
            {
              path: 'src/session.rs',
              score: 0.95,
              level: 1,
              provenance: ['symbol: SessionManager'],
              content: 'pub struct SessionManager;',
            },
          ],
          excluded_files: [],
        };

        const res = client.validateContextReport(raw);
        assert.strictEqual(res.task, 'Fix race condition in session manager');
        assert.strictEqual(res.project, 'AMstraLift');
        assert.strictEqual(res.confidence, 'HIGH');
        assert.strictEqual(res.files.length, 1);
        assert.strictEqual(res.files[0].path, 'src/session.rs');
        assert.strictEqual(res.files[0].score, 0.95);
      });

      it('should throw when context report schema is invalid', () => {
        assert.throws(() => client.validateContextReport(null), /not a JSON object/);
        assert.throws(() => client.validateContextReport({}), /Missing or invalid "task"/);
        assert.throws(
          () => client.validateContextReport({ task: 'foo', files: 'invalid' }),
          /Missing or invalid "files" array/
        );
      });
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

    describe('Real CLI Invocations: listMemories', () => {
      it('should list memories on real Engram workspace', async function () {
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const memories = await client.listMemories(debugBin, repoRoot);
        assert.ok(Array.isArray(memories));
      });

      it('should return empty list on uninitialized workspace directory', async function () {
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram-mem-test-'));
        try {
          const memories = await client.listMemories(debugBin, tempDir);
          assert.deepStrictEqual(memories, []);
        } finally {
          fs.rmSync(tempDir, { recursive: true, force: true });
        }
      });
    });

    describe('Real CLI Invocations: compileContext', () => {
      it('should compile context for a task on real Engram repository', async function () {
        this.timeout(65000);
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const report = await client.compileContext(
          debugBin,
          repoRoot,
          'Add retry logic to payment service',
          1200,
          undefined,
          60000
        );
        assert.strictEqual(report.task, 'Add retry logic to payment service');
        assert.strictEqual(report.token_budget, 1200);
        assert.ok(typeof report.estimated_tokens === 'number');
        assert.ok(Array.isArray(report.files));
      });

      it('should compile context gracefully on uninitialized directory', async function () {
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram-ctx-test-'));
        try {
          const report = await client.compileContext(
            debugBin,
            tempDir,
            'Test empty workspace context compilation',
            1000
          );
          assert.strictEqual(report.task, 'Test empty workspace context compilation');
          assert.strictEqual(report.files.length, 0);
          assert.strictEqual(report.estimated_tokens, 0);
        } finally {
          fs.rmSync(tempDir, { recursive: true, force: true });
        }
      });

      it('RELIABILITY: should cancel context compilation when requested', async function () {
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const cts = new vscode.CancellationTokenSource();
        const promise = client.compileContext(
          debugBin,
          repoRoot,
          'Task for cancellation test',
          1200,
          cts.token
        );
        cts.cancel();

        await assert.rejects(
          promise,
          /Operation cancelled by user/
        );
      });

      it('RELIABILITY: should handle context compilation timeout cleanly', async function () {
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        await assert.rejects(
          async () => {
            await client.compileContext(
              debugBin,
              repoRoot,
              'Task for timeout test',
              1200,
              undefined,
              1
            );
          },
          /Command timed out after 1ms/
        );
      });

      it('RELIABILITY: should kill child process on cancellation without leaving orphans', async function () {
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const cts = new vscode.CancellationTokenSource();

        const promise = client.compileContext(
          debugBin,
          repoRoot,
          'Task for cancellation orphan check',
          1200,
          cts.token,
          30000
        );

        // Wait brief delay for child process to be spawned
        await new Promise((r) => setTimeout(r, 60));
        cts.cancel();

        await assert.rejects(promise, /Operation cancelled by user/);

        // Allow OS to finalize termination
        await new Promise((r) => setTimeout(r, 600));

        if (process.platform === 'win32') {
          const check = cp.spawnSync('powershell.exe', [
            '-NoProfile',
            '-Command',
            'Get-Process engram -ErrorAction SilentlyContinue',
          ]);
          assert.strictEqual(
            check.stdout.toString().trim(),
            '',
            'No engram process should remain active after cancellation'
          );
        }
      });

      it('should reject concurrent compileContext invocations', async function () {
        this.timeout(45000);
        if (!fs.existsSync(debugBin)) {
          this.skip();
          return;
        }

        const p1 = client.compileContext(
          debugBin,
          repoRoot,
          'Task 1 for concurrency test',
          1200
        );

        await assert.rejects(
          async () => {
            await client.compileContext(
              debugBin,
              repoRoot,
              'Task 2 for concurrency test',
              1200
            );
          },
          /A context compilation is already in progress/
        );

        await p1;
      });
    });
  });
});
