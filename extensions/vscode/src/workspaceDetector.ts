import * as fs from 'fs';
import * as path from 'path';
import { ProjectSignature, WorkspaceInfo } from './types';

export class WorkspaceDetector {
  /**
   * Detects project characteristics, framework signatures, and Engram initialization state.
   */
  public detect(workspaceRoot: string): WorkspaceInfo {
    const detected: ProjectSignature[] = [];
    let isInitialized = false;
    let hasConfig = false;

    try {
      if (!fs.existsSync(workspaceRoot)) {
        return {
          root: workspaceRoot,
          isInitialized: false,
          detectedProjects: [],
          hasConfig: false,
        };
      }

      // Check for Engram initialization directory
      const engramDir = path.join(workspaceRoot, '.engram');
      if (fs.existsSync(engramDir)) {
        try {
          const stat = fs.statSync(engramDir);
          isInitialized = stat.isDirectory();
        } catch {
          isInitialized = false;
        }
      }

      // Check for MCP configuration in .vscode/mcp.json
      const mcpJson = path.join(workspaceRoot, '.vscode', 'mcp.json');
      if (fs.existsSync(mcpJson)) {
        hasConfig = true;
      }

      // Inspect directory entries (top-level only to remain fast and bounded)
      const entries = fs.readdirSync(workspaceRoot);
      const entrySet = new Set(entries.map((e) => e.toLowerCase()));

      // 1. Angular
      if (entrySet.has('angular.json')) {
        detected.push('angular');
      }

      // 2. .NET (.sln, .csproj, global.json)
      const hasDotnet =
        entrySet.has('global.json') ||
        entries.some((e) => e.endsWith('.sln') || e.endsWith('.csproj') || e.endsWith('.fsproj'));
      if (hasDotnet) {
        detected.push('dotnet');
      }

      // 3. Python (pyproject.toml, requirements.txt, Pipfile, setup.py)
      if (
        entrySet.has('pyproject.toml') ||
        entrySet.has('requirements.txt') ||
        entrySet.has('pipfile') ||
        entrySet.has('setup.py')
      ) {
        detected.push('python');
      }

      // 4. Rust (Cargo.toml)
      if (entrySet.has('cargo.toml')) {
        detected.push('rust');
      }

      // 5. Go (go.mod)
      if (entrySet.has('go.mod')) {
        detected.push('go');
      }

      // 6. Java (pom.xml, build.gradle, build.gradle.kts)
      if (
        entrySet.has('pom.xml') ||
        entrySet.has('build.gradle') ||
        entrySet.has('build.gradle.kts')
      ) {
        detected.push('java');
      }

      // 7. TypeScript / JavaScript
      if (entrySet.has('tsconfig.json')) {
        detected.push('typescript');
      } else if (entrySet.has('package.json') && !detected.includes('angular')) {
        detected.push('typescript');
      }

      if (detected.length === 0) {
        detected.push('generic');
      }
    } catch {
      // In case of I/O error or permission issues, return safe default
      if (detected.length === 0) {
        detected.push('generic');
      }
    }

    return {
      root: workspaceRoot,
      isInitialized,
      detectedProjects: detected,
      hasConfig,
    };
  }
}
