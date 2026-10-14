import * as assert from 'assert';
import { EngramStatusBar } from '../src/statusBar';
import { BinaryResolutionResult } from '../src/types';

describe('EngramStatusBar Tests', () => {
  let statusBar: EngramStatusBar = new EngramStatusBar();

  beforeEach(() => {
    statusBar = new EngramStatusBar();
  });

  afterEach(() => {
    statusBar.dispose();
  });

  it('should initialize in pending state', () => {
    const rawItem = (statusBar as any).statusBarItem;
    assert.strictEqual(rawItem.text, '$(sync~spin) Engram: Initializing...');
    assert.strictEqual(rawItem.command, 'engram.locateBinary');
    assert.strictEqual(rawItem.visible, true);
  });

  it('should update to active status when binary resolution succeeds', () => {
    const rawItem = (statusBar as any).statusBarItem;
    const successResult: BinaryResolutionResult = {
      valid: true,
      path: 'C:\\bin\\engram.exe',
      version: '1.4.0',
      source: 'standard_location',
    };

    statusBar.updateWithResolution(successResult);
    assert.strictEqual(rawItem.text, '$(shield) Engram v1.4.0');
    assert.strictEqual(rawItem.command, 'engram.checkHealth');
    assert.ok(rawItem.tooltip.value.includes('C:\\bin\\engram.exe'));
    assert.ok(rawItem.tooltip.value.includes('1.4.0'));
  });

  it('should update to warning status when binary resolution fails', () => {
    const rawItem = (statusBar as any).statusBarItem;
    const failureResult: BinaryResolutionResult = {
      valid: false,
      error: 'Engram binary not found in standard system locations.',
      isExplicitConfigError: false,
    };

    statusBar.updateWithResolution(failureResult);
    assert.strictEqual(rawItem.text, '$(alert) Engram: Not Found');
    assert.strictEqual(rawItem.command, 'engram.locateBinary');
    assert.ok(rawItem.backgroundColor !== undefined);
    assert.ok(rawItem.tooltip.value.includes('Engram Binary Missing or Incompatible'));
  });

  it('should update error status', () => {
    const rawItem = (statusBar as any).statusBarItem;
    statusBar.setError('Connection timeout');
    assert.strictEqual(rawItem.text, '$(error) Engram: Error');
    assert.strictEqual(rawItem.command, 'engram.checkHealth');
    assert.ok(rawItem.tooltip.includes('Connection timeout'));
  });

  it('should set health result tooltip correctly', () => {
    const rawItem = (statusBar as any).statusBarItem;
    statusBar.setHealthResult(42, 40, true);
    assert.ok(rawItem.tooltip.value.includes('Total Memories: 42'));
    assert.ok(rawItem.tooltip.value.includes('Active / Live: 40'));
    assert.ok(rawItem.tooltip.value.includes('Provenance Hash Chain: ✓'));
  });
});
