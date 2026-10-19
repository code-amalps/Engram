import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import { BinaryResolver } from '../src/binaryResolver';

function findExtensionRoot(startDir: string): string {
  let cur = startDir;
  while (cur !== path.dirname(cur)) {
    if (fs.existsSync(path.join(cur, 'package.json')) && !fs.existsSync(path.join(cur, 'Cargo.toml'))) {
      return cur;
    }
    cur = path.dirname(cur);
  }
  return startDir;
}

const extensionRoot = findExtensionRoot(__dirname);
const dummyPath = path.join(extensionRoot, 'package.json');

describe('BinaryResolver Tests', () => {
  let resolver: BinaryResolver = new BinaryResolver();

  beforeEach(() => {
    resolver = new BinaryResolver();
  });

  describe('Semver Parsing & Compatibility Logic', () => {
    it('should parse standard semver strings', () => {
      assert.deepStrictEqual(resolver.parseSemVer('1.4.0'), [1, 4, 0]);
      assert.deepStrictEqual(resolver.parseSemVer('1.5.1'), [1, 5, 1]);
      assert.deepStrictEqual(resolver.parseSemVer('2.0.0-alpha'), [2, 0, 0]);
    });

    it('should return null for malformed semver strings', () => {
      assert.strictEqual(resolver.parseSemVer('invalid'), null);
      assert.strictEqual(resolver.parseSemVer('1.4'), null);
      assert.strictEqual(resolver.parseSemVer(''), null);
    });

    it('should accept compatible versions in range [1.4.0, 2.0.0)', () => {
      assert.strictEqual(resolver.isCompatibleVersion([1, 4, 0]), true);
      assert.strictEqual(resolver.isCompatibleVersion([1, 4, 1]), true);
      assert.strictEqual(resolver.isCompatibleVersion([1, 5, 0]), true);
      assert.strictEqual(resolver.isCompatibleVersion([1, 9, 9]), true);
    });

    it('should reject versions below 1.4.0', () => {
      assert.strictEqual(resolver.isCompatibleVersion([1, 3, 9]), false);
      assert.strictEqual(resolver.isCompatibleVersion([1, 2, 0]), false);
      assert.strictEqual(resolver.isCompatibleVersion([0, 9, 0]), false);
    });

    it('should reject versions at or above 2.0.0', () => {
      assert.strictEqual(resolver.isCompatibleVersion([2, 0, 0]), false);
      assert.strictEqual(resolver.isCompatibleVersion([2, 1, 0]), false);
      assert.strictEqual(resolver.isCompatibleVersion([3, 0, 0]), false);
    });
  });

  describe('Binary Verification with Controlled Runner', () => {
    it('should succeed with valid engram 1.4.0 output', async () => {
      const mockRunner = async () => ({
        stdout: 'engram 1.4.0\n',
        stderr: '',
        code: 0,
      });
      const customResolver = new BinaryResolver(mockRunner);

      const res = await customResolver.verifyBinary(dummyPath, 'config');
      assert.strictEqual(res.valid, true);
      if (res.valid) {
        assert.strictEqual(res.version, '1.4.0');
        assert.strictEqual(res.path, dummyPath);
      }
    });

    it('should reject older versions with actionable upgrade error', async () => {
      const mockRunner = async () => ({
        stdout: 'engram 1.2.0\n',
        stderr: '',
        code: 0,
      });
      const customResolver = new BinaryResolver(mockRunner);
      const res = await customResolver.verifyBinary(dummyPath, 'config');
      assert.strictEqual(res.valid, false);
      if (!res.valid) {
        assert.match(res.error, /incompatible/i);
        assert.match(res.error, /Please upgrade Engram/i);
      }
    });

    it('should reject malformed version output', async () => {
      const mockRunner = async () => ({
        stdout: 'hello world from custom binary\n',
        stderr: '',
        code: 0,
      });
      const customResolver = new BinaryResolver(mockRunner);
      const res = await customResolver.verifyBinary(dummyPath, 'config');
      assert.strictEqual(res.valid, false);
      if (!res.valid) {
        assert.match(res.error, /Malformed version output/i);
      }
    });

    it('should handle non-existent binary path cleanly', async () => {
      const nonExistent = path.join(__dirname, 'non_existent_engram_binary_xyz.exe');
      const res = await resolver.verifyBinary(nonExistent, 'config');
      assert.strictEqual(res.valid, false);
      if (!res.valid) {
        assert.match(res.error, /File does not exist/i);
      }
    });
  });

  describe('Precedence & Zero-Fallback Invariant', () => {
    it('CRITICAL: Never fall back when explicit engram.binaryPath is invalid', async () => {
      // Mock runner returns success for system binaries, but explicit path points to non-existent file
      const nonExistent = 'C:\\Definitely\\Invalid\\Path\\engram.exe';
      const res = await resolver.resolve(nonExistent);

      assert.strictEqual(res.valid, false);
      if (!res.valid) {
        assert.strictEqual(res.isExplicitConfigError, true);
        assert.match(res.error, /Explicitly configured binary path/i);
      }
    });

    it('should resolve via ENGRAM_PATH environment variable', async () => {
      const originalEnv = process.env.ENGRAM_PATH;
      try {
        process.env.ENGRAM_PATH = dummyPath;

        const mockRunner = async () => ({
          stdout: 'engram 1.4.0\n',
          stderr: '',
          code: 0,
        });
        const customResolver = new BinaryResolver(mockRunner);
        const res = await customResolver.resolve();

        assert.strictEqual(res.valid, true);
        if (res.valid) {
          assert.strictEqual(res.source, 'env_engram_path');
        }
      } finally {
        if (originalEnv !== undefined) {
          process.env.ENGRAM_PATH = originalEnv;
        } else {
          delete process.env.ENGRAM_PATH;
        }
      }
    });

    it('should resolve via ENGRAM_INSTALL_DIR environment variable', async () => {
      const originalEnv = process.env.ENGRAM_INSTALL_DIR;
      const originalEngramPath = process.env.ENGRAM_PATH;
      delete process.env.ENGRAM_PATH;

      const tempDir = path.join(__dirname, 'temp_install_test');
      const binDir = path.join(tempDir, 'bin');
      const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
      const fakeBin = path.join(binDir, binName);

      try {
        fs.mkdirSync(binDir, { recursive: true });
        fs.writeFileSync(fakeBin, 'dummy binary content');
        process.env.ENGRAM_INSTALL_DIR = tempDir;

        const mockRunner = async () => ({
          stdout: 'engram 1.4.0\n',
          stderr: '',
          code: 0,
        });
        const customResolver = new BinaryResolver(mockRunner);
        const res = await customResolver.resolve();

        assert.strictEqual(res.valid, true);
        if (res.valid) {
          assert.strictEqual(res.source, 'env_install_dir');
        }
      } finally {
        if (originalEnv !== undefined) {
          process.env.ENGRAM_INSTALL_DIR = originalEnv;
        } else {
          delete process.env.ENGRAM_INSTALL_DIR;
        }
        if (originalEngramPath !== undefined) {
          process.env.ENGRAM_PATH = originalEngramPath;
        }
        if (fs.existsSync(tempDir)) {
          fs.rmSync(tempDir, { recursive: true, force: true });
        }
      }
    });

    it('should resolve actual local target/debug/engram executable if present', async () => {
      const binName = process.platform === 'win32' ? 'engram.exe' : 'engram';
      const debugBin = path.join(extensionRoot, '..', '..', 'target', 'debug', binName);

      if (fs.existsSync(debugBin)) {
        const repoRootDir = path.resolve(extensionRoot, '..', '..');
        const res = await resolver.resolve(undefined, repoRootDir);
        assert.strictEqual(res.valid, true);
        if (res.valid) {
          const semver = resolver.parseSemVer(res.version);
          assert.ok(semver !== null, `Parsed semver must not be null: ${res.version}`);
          assert.ok(resolver.isCompatibleVersion(semver!), `Version ${res.version} must be compatible`);
        }
      }
    });

    it('CROSS-PLATFORM: should compute correct standard search paths across platforms', () => {
      const origPlatform = process.platform;
      const origHome = process.env.HOME;
      const origLocalAppData = process.env.LOCALAPPDATA;
      const origProgramFiles = process.env.ProgramFiles;

      try {
        // Test macOS / Linux POSIX standard paths
        Object.defineProperty(process, 'platform', { value: 'darwin' });
        process.env.HOME = '/Users/testuser';
        const macPaths = resolver.getStandardPlatformPaths('engram');
        assert.ok(macPaths.includes('/usr/local/bin/engram'));
        assert.ok(macPaths.includes('/opt/homebrew/bin/engram'));
        assert.ok(macPaths.includes(path.join('/Users/testuser', '.local', 'bin', 'engram')));

        Object.defineProperty(process, 'platform', { value: 'linux' });
        process.env.HOME = '/home/testuser';
        const linuxPaths = resolver.getStandardPlatformPaths('engram');
        assert.ok(linuxPaths.includes('/usr/local/bin/engram'));
        assert.ok(linuxPaths.includes('/opt/homebrew/bin/engram'));
        assert.ok(linuxPaths.includes(path.join('/home/testuser', '.local', 'bin', 'engram')));

        // Test Windows standard paths
        Object.defineProperty(process, 'platform', { value: 'win32' });
        process.env.LOCALAPPDATA = 'C:\\Users\\testuser\\AppData\\Local';
        process.env.ProgramFiles = 'C:\\Program Files';
        const winPaths = resolver.getStandardPlatformPaths('engram.exe');
        assert.ok(winPaths.includes('C:\\Users\\testuser\\AppData\\Local\\Programs\\Engram\\bin\\engram.exe'));
        assert.ok(winPaths.includes('C:\\Program Files\\Engram\\bin\\engram.exe'));
      } finally {
        Object.defineProperty(process, 'platform', { value: origPlatform });
        if (origHome !== undefined) process.env.HOME = origHome; else delete process.env.HOME;
        if (origLocalAppData !== undefined) process.env.LOCALAPPDATA = origLocalAppData; else delete process.env.LOCALAPPDATA;
        if (origProgramFiles !== undefined) process.env.ProgramFiles = origProgramFiles; else delete process.env.ProgramFiles;
      }
    });
  });
});
