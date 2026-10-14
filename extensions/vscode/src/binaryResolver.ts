import * as fs from 'fs';
import * as path from 'path';
import * as cp from 'child_process';
import {
  BinaryResolutionResult,
  BinaryResolutionSource,
  BinaryResolutionSuccess,
  BinaryResolutionFailure,
} from './types';

export const MIN_COMPATIBLE_VERSION = [1, 4, 0];
export const MAX_EXCLUSIVE_VERSION = [2, 0, 0];

export type VersionRunner = (executablePath: string) => Promise<{ stdout: string; stderr: string; code: number }>;

export class BinaryResolver {
  private cachedResult: BinaryResolutionSuccess | null = null;
  private versionRunner: VersionRunner;

  constructor(customRunner?: VersionRunner) {
    this.versionRunner = customRunner || this.defaultVersionRunner;
  }

  public clearCache(): void {
    this.cachedResult = null;
  }

  /**
   * Resolves the Engram binary based on strict precedence rules:
   * 1. Explicit `engram.binaryPath` configuration setting (NEVER falls back if invalid)
   * 2. ENGRAM_PATH environment variable
   * 3. ENGRAM_INSTALL_DIR environment variable
   * 4. System PATH
   * 5. OS standard installation locations
   * 6. Developer toolchain fallbacks
   */
  public async resolve(explicitSetting?: string, workspaceRoot?: string): Promise<BinaryResolutionResult> {
    if (this.cachedResult && !explicitSetting) {
      return this.cachedResult;
    }

    const binaryName = process.platform === 'win32' ? 'engram.exe' : 'engram';

    // 1. Explicit Configuration Setting
    if (explicitSetting && explicitSetting.trim() !== '') {
      const trimmed = explicitSetting.trim();
      const verified = await this.verifyBinary(trimmed, 'config');
      if (verified.valid) {
        this.cachedResult = verified;
        return verified;
      }
      // CRITICAL SAFEGUARD: Never fall back if explicit configuration is provided!
      return {
        valid: false,
        attemptedPath: trimmed,
        error: `Explicitly configured binary path '${trimmed}' is invalid or incompatible: ${verified.error}`,
        source: 'config',
        isExplicitConfigError: true,
      };
    }

    // 2. ENGRAM_PATH Environment Variable
    if (process.env.ENGRAM_PATH && process.env.ENGRAM_PATH.trim() !== '') {
      const envPath = process.env.ENGRAM_PATH.trim();
      const verified = await this.verifyBinary(envPath, 'env_engram_path');
      if (verified.valid) {
        this.cachedResult = verified;
        return verified;
      }
    }

    // 3. ENGRAM_INSTALL_DIR Environment Variable
    if (process.env.ENGRAM_INSTALL_DIR && process.env.ENGRAM_INSTALL_DIR.trim() !== '') {
      const installDirBin = path.join(process.env.ENGRAM_INSTALL_DIR.trim(), 'bin', binaryName);
      const verified = await this.verifyBinary(installDirBin, 'env_install_dir');
      if (verified.valid) {
        this.cachedResult = verified;
        return verified;
      }
    }

    // 4. System PATH
    const pathCandidates = this.findInSystemPath(binaryName);
    for (const candidate of pathCandidates) {
      const verified = await this.verifyBinary(candidate, 'path');
      if (verified.valid) {
        this.cachedResult = verified;
        return verified;
      }
    }

    // 5. Standard Platform Installation Locations
    const standardCandidates = this.getStandardPlatformPaths(binaryName);
    for (const candidate of standardCandidates) {
      const verified = await this.verifyBinary(candidate, 'standard_location');
      if (verified.valid) {
        this.cachedResult = verified;
        return verified;
      }
    }

    // 6. Developer Fallbacks
    const devCandidates = this.getDeveloperFallbackPaths(binaryName, workspaceRoot);
    for (const candidate of devCandidates) {
      const verified = await this.verifyBinary(candidate, 'developer_fallback');
      if (verified.valid) {
        this.cachedResult = verified;
        return verified;
      }
    }

    return {
      valid: false,
      error: 'Engram executable was not found. Please install Engram or configure engram.binaryPath.',
      isExplicitConfigError: false,
    };
  }

  /**
   * Verifies an executable by checking existence, executing --version, and checking semver.
   */
  public async verifyBinary(filePath: string, source: BinaryResolutionSource): Promise<BinaryResolutionResult> {
    try {
      if (!fs.existsSync(filePath)) {
        return {
          valid: false,
          attemptedPath: filePath,
          error: `File does not exist at '${filePath}'`,
          source,
          isExplicitConfigError: false,
        };
      }

      const stat = fs.statSync(filePath);
      if (!stat.isFile()) {
        return {
          valid: false,
          attemptedPath: filePath,
          error: `Path '${filePath}' is not a regular file`,
          source,
          isExplicitConfigError: false,
        };
      }

      // Execute <binary> --version safely
      const { stdout, stderr, code } = await this.versionRunner(filePath);
      if (code !== 0) {
        return {
          valid: false,
          attemptedPath: filePath,
          error: `Command '${filePath} --version' failed with code ${code}: ${stderr.trim() || stdout.trim()}`,
          source,
          isExplicitConfigError: false,
        };
      }

      const output = stdout.trim();
      const versionMatch = output.match(/^engram\s+v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)/i);
      if (!versionMatch) {
        return {
          valid: false,
          attemptedPath: filePath,
          error: `Malformed version output: '${output}'. Expected format 'engram X.Y.Z'.`,
          source,
          isExplicitConfigError: false,
        };
      }

      const versionString = versionMatch[1];
      const parsedVersion = this.parseSemVer(versionString);
      if (!parsedVersion) {
        return {
          valid: false,
          attemptedPath: filePath,
          error: `Could not parse semver from version string '${versionString}'`,
          source,
          isExplicitConfigError: false,
        };
      }

      if (!this.isCompatibleVersion(parsedVersion)) {
        return {
          valid: false,
          attemptedPath: filePath,
          error: `Engram version ${versionString} is incompatible. Requires >= 1.4.0 and < 2.0.0. Please upgrade Engram.`,
          source,
          isExplicitConfigError: false,
        };
      }

      return {
        valid: true,
        path: filePath,
        version: versionString,
        source,
      };
    } catch (err: any) {
      return {
        valid: false,
        attemptedPath: filePath,
        error: `Failed to inspect executable '${filePath}': ${err.message || String(err)}`,
        source,
        isExplicitConfigError: false,
      };
    }
  }

  public parseSemVer(versionStr: string): [number, number, number] | null {
    const parts = versionStr.split('-')[0].split('.');
    if (parts.length < 3) {
      return null;
    }
    const major = parseInt(parts[0], 10);
    const minor = parseInt(parts[1], 10);
    const patch = parseInt(parts[2], 10);
    if (isNaN(major) || isNaN(minor) || isNaN(patch)) {
      return null;
    }
    return [major, minor, patch];
  }

  public isCompatibleVersion(version: [number, number, number]): boolean {
    const [maj, min, pat] = version;
    const [minMaj, minMin, minPat] = MIN_COMPATIBLE_VERSION;
    const [maxMaj, maxMin, maxPat] = MAX_EXCLUSIVE_VERSION;

    // Check >= MIN
    if (maj < minMaj) {
      return false;
    }
    if (maj === minMaj && min < minMin) {
      return false;
    }
    if (maj === minMaj && min === minMin && pat < minPat) {
      return false;
    }

    // Check < MAX
    if (maj > maxMaj) {
      return false;
    }
    if (maj === maxMaj && min > maxMin) {
      return false;
    }
    if (maj === maxMaj && min === maxMin && pat >= maxPat) {
      return false;
    }

    return true;
  }

  public findInSystemPath(binaryName: string): string[] {
    const rawPath = process.env.PATH || '';
    const delimiter = process.platform === 'win32' ? ';' : ':';
    const dirs = rawPath.split(delimiter).filter((d) => d.trim().length > 0);
    const results: string[] = [];

    for (const dir of dirs) {
      const fullPath = path.join(dir, binaryName);
      if (fs.existsSync(fullPath)) {
        results.push(fullPath);
      }
    }
    return results;
  }

  public getStandardPlatformPaths(binaryName: string): string[] {
    const paths: string[] = [];

    if (process.platform === 'win32') {
      const localAppData = process.env.LOCALAPPDATA;
      if (localAppData) {
        paths.push(path.join(localAppData, 'Programs', 'Engram', 'bin', binaryName));
      }
      const programFiles = process.env.ProgramFiles;
      if (programFiles) {
        paths.push(path.join(programFiles, 'Engram', 'bin', binaryName));
      }
    } else {
      // macOS / Linux
      paths.push(`/usr/local/bin/${binaryName}`);
      paths.push(`/opt/homebrew/bin/${binaryName}`);
      const home = process.env.HOME;
      if (home) {
        paths.push(path.join(home, '.local', 'bin', binaryName));
      }
    }

    return paths;
  }

  public getDeveloperFallbackPaths(binaryName: string, workspaceRoot?: string): string[] {
    const paths: string[] = [];

    // ~/.cargo/bin
    const userProfile = process.platform === 'win32' ? process.env.USERPROFILE : process.env.HOME;
    if (userProfile) {
      paths.push(path.join(userProfile, '.cargo', 'bin', binaryName));
    }

    // Local target directories if in repository workspace
    if (workspaceRoot) {
      paths.push(path.join(workspaceRoot, 'target', 'release', binaryName));
      paths.push(path.join(workspaceRoot, 'target', 'debug', binaryName));
    }

    return paths;
  }

  private defaultVersionRunner(executablePath: string): Promise<{ stdout: string; stderr: string; code: number }> {
    return new Promise((resolve) => {
      cp.execFile(
        executablePath,
        ['--version'],
        {
          shell: false,
          timeout: 5000,
          windowsHide: true,
        },
        (error, stdout, stderr) => {
          resolve({
            stdout: stdout || '',
            stderr: stderr || '',
            code: error ? (error.code as number) || 1 : 0,
          });
        }
      );
    });
  }
}
