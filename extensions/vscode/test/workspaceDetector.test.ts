import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { WorkspaceDetector } from '../src/workspaceDetector';

describe('WorkspaceDetector Tests', () => {
  let detector: WorkspaceDetector;
  let tempDir: string;

  beforeEach(() => {
    detector = new WorkspaceDetector();
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'engram-ws-test-'));
  });

  afterEach(() => {
    try {
      fs.rmSync(tempDir, { recursive: true, force: true });
    } catch {
      // ignore
    }
  });

  it('should detect Angular project from angular.json', () => {
    fs.writeFileSync(path.join(tempDir, 'angular.json'), '{}', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('angular'));
    assert.strictEqual(info.isInitialized, false);
    assert.strictEqual(info.hasConfig, false);
  });

  it('should detect .NET project from .csproj and .sln', () => {
    fs.writeFileSync(path.join(tempDir, 'MyService.csproj'), '<Project></Project>', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('dotnet'));
  });

  it('should detect Python project from pyproject.toml and requirements.txt', () => {
    fs.writeFileSync(path.join(tempDir, 'pyproject.toml'), '[tool.poetry]', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('python'));
  });

  it('should detect Rust project from Cargo.toml', () => {
    fs.writeFileSync(path.join(tempDir, 'Cargo.toml'), '[package]', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('rust'));
  });

  it('should detect Go project from go.mod', () => {
    fs.writeFileSync(path.join(tempDir, 'go.mod'), 'module test', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('go'));
  });

  it('should detect Java project from pom.xml and build.gradle', () => {
    fs.writeFileSync(path.join(tempDir, 'pom.xml'), '<project></project>', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('java'));
  });

  it('should detect TypeScript project from tsconfig.json', () => {
    fs.writeFileSync(path.join(tempDir, 'tsconfig.json'), '{}', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('typescript'));
  });

  it('should detect polyglot workspace with multiple project frameworks', () => {
    fs.writeFileSync(path.join(tempDir, 'pyproject.toml'), '', 'utf8');
    fs.writeFileSync(path.join(tempDir, 'tsconfig.json'), '', 'utf8');
    const info = detector.detect(tempDir);
    assert.ok(info.detectedProjects.includes('python'));
    assert.ok(info.detectedProjects.includes('typescript'));
  });

  it('should detect .engram directory initialization', () => {
    fs.mkdirSync(path.join(tempDir, '.engram'));
    const info = detector.detect(tempDir);
    assert.strictEqual(info.isInitialized, true);
  });

  it('should detect .vscode/mcp.json presence', () => {
    fs.mkdirSync(path.join(tempDir, '.vscode'));
    fs.writeFileSync(path.join(tempDir, '.vscode', 'mcp.json'), '{}', 'utf8');
    const info = detector.detect(tempDir);
    assert.strictEqual(info.hasConfig, true);
  });

  it('should return generic for empty workspace directory cleanly', () => {
    const info = detector.detect(tempDir);
    assert.deepStrictEqual(info.detectedProjects, ['generic']);
    assert.strictEqual(info.isInitialized, false);
    assert.strictEqual(info.hasConfig, false);
  });

  it('should handle nonexistent directory without throwing', () => {
    const nonExistent = path.join(tempDir, 'does_not_exist_folder');
    const info = detector.detect(nonExistent);
    assert.strictEqual(info.isInitialized, false);
    assert.deepStrictEqual(info.detectedProjects, []);
  });
});
