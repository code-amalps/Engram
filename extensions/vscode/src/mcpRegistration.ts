import * as fs from 'fs';
import * as path from 'path';
import { McpRegistrationStatus } from './types';

export interface McpServerDefinition {
  type?: string;
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

export interface McpConfigFile {
  servers?: Record<string, McpServerDefinition>;
  mcpServers?: Record<string, McpServerDefinition>;
  [key: string]: any;
}

export interface RegistrationInspectResult {
  status: McpRegistrationStatus;
  path: string;
  details?: string;
  existingDefinition?: McpServerDefinition;
}

export class McpRegistrationManager {
  /**
   * Returns the canonical path to the project's .vscode/mcp.json.
   */
  public getConfigPath(workspaceRoot: string): string {
    return path.join(workspaceRoot, '.vscode', 'mcp.json');
  }

  /**
   * Returns the canonical path to the root .mcp.json used by Copilot Agent harness and external agents.
   */
  public getRootConfigPath(workspaceRoot: string): string {
    return path.join(workspaceRoot, '.mcp.json');
  }

  /**
   * Safely strips line and block comments from JSON text to permit valid parsing.
   */
  public parseJsonWithComments(rawText: string): any {
    // Strip /* ... */ block comments and // ... line comments outside of strings
    const stripped = rawText.replace(
      /\\"|"(?:\\"|[^"])*"|(\/\/.*|\/\*[\s\S]*?\*\/)/g,
      (match, group) => (group ? '' : match)
    );
    return JSON.parse(stripped);
  }

  /**
   * Inspects the current registration status in .vscode/mcp.json and root .mcp.json.
   */
  public inspect(workspaceRoot: string, expectedExecutable?: string): RegistrationInspectResult {
    const configPath = this.getConfigPath(workspaceRoot);
    const rootConfigPath = this.getRootConfigPath(workspaceRoot);

    const hasVsCodeConfig = fs.existsSync(configPath);
    const hasRootConfig = fs.existsSync(rootConfigPath);

    if (!hasVsCodeConfig && !hasRootConfig) {
      return {
        status: 'NOT_CONFIGURED',
        path: configPath,
        details: 'Configuration file .vscode/mcp.json does not exist.',
      };
    }

    // Inspect candidate file (prefer .vscode/mcp.json, fallback to root .mcp.json)
    const targetFile = hasVsCodeConfig ? configPath : rootConfigPath;
    let parsed: McpConfigFile;
    try {
      const text = fs.readFileSync(targetFile, 'utf8');
      parsed = this.parseJsonWithComments(text);
    } catch (err: any) {
      return {
        status: 'MALFORMED_JSON',
        path: targetFile,
        details: `Failed to parse ${path.basename(targetFile)}: ${err.message || String(err)}`,
      };
    }

    if (!parsed || typeof parsed !== 'object') {
      return {
        status: 'MALFORMED_JSON',
        path: targetFile,
        details: `Root of ${path.basename(targetFile)} is not an object.`,
      };
    }

    // Check both native 'servers' schema and portable 'mcpServers' schema
    let engramDef = parsed.servers?.['engram'] || parsed.mcpServers?.['engram'];

    // If engram wasn't in .vscode/mcp.json, check root .mcp.json
    if (!engramDef && hasVsCodeConfig && hasRootConfig) {
      try {
        const rootText = fs.readFileSync(rootConfigPath, 'utf8');
        const rootParsed: McpConfigFile = this.parseJsonWithComments(rootText);
        engramDef = rootParsed.servers?.['engram'] || rootParsed.mcpServers?.['engram'];
      } catch {
        // ignore fallback errors
      }
    }

    if (!engramDef) {
      return {
        status: 'NOT_CONFIGURED',
        path: targetFile,
        details: `No "engram" server entry found in ${path.basename(targetFile)}.`,
      };
    }

    const command = engramDef.command || '';
    const args = Array.isArray(engramDef.args) ? engramDef.args : [];

    // Verify command
    const cmdMatches =
      !expectedExecutable ||
      command === expectedExecutable ||
      path.basename(command).toLowerCase() === path.basename(expectedExecutable).toLowerCase() ||
      command === 'engram' ||
      command === 'engram.exe';

    // Verify args: expect ["-C", "${workspaceFolder}" | workspaceRoot, "mcp"]
    const hasDashC = args.includes('-C');
    const hasMcp = args.includes('mcp');
    const hasValidRoot =
      args.includes('${workspaceFolder}') ||
      args.some((a) => path.resolve(a) === path.resolve(workspaceRoot));

    const isHealthy = cmdMatches && hasDashC && hasMcp && hasValidRoot;

    if (isHealthy) {
      return {
        status: 'CONFIGURED_HEALTHY',
        path: targetFile,
        existingDefinition: engramDef,
      };
    }

    return {
      status: 'CONFLICT',
      path: targetFile,
      details: `Existing server configuration does not match expected parameters. Command: '${command}', Args: ${JSON.stringify(args)}`,
      existingDefinition: engramDef,
    };
  }

  /**
   * Idempotently registers or repairs the Engram server in .vscode/mcp.json and root .mcp.json.
   * Preserves all other servers and user configuration.
   */
  public register(
    workspaceRoot: string,
    executablePath?: string,
    force = false
  ): { success: boolean; changed: boolean; error?: string } {
    const configPath = this.getConfigPath(workspaceRoot);
    const rootConfigPath = this.getRootConfigPath(workspaceRoot);
    const vscodeDir = path.dirname(configPath);

    let changed = false;

    try {
      // Determine command string: prefer "engram" if no custom path given or if standard
      const command = executablePath && executablePath.trim() !== '' ? executablePath.trim() : 'engram';

      const vsCodeServerBlock: McpServerDefinition = {
        type: 'stdio',
        command,
        args: ['-C', '${workspaceFolder}', 'mcp'],
      };

      const rootServerBlock: McpServerDefinition = {
        command,
        args: ['-C', '${workspaceFolder}', 'mcp'],
      };

      // 1. Write/update .vscode/mcp.json (VS Code native "servers" schema)
      if (!fs.existsSync(vscodeDir)) {
        fs.mkdirSync(vscodeDir, { recursive: true });
      }

      let parsed: McpConfigFile = {};
      if (fs.existsSync(configPath)) {
        try {
          const text = fs.readFileSync(configPath, 'utf8');
          parsed = this.parseJsonWithComments(text);
          if (!parsed || typeof parsed !== 'object') {
            parsed = {};
          }
        } catch (err: any) {
          if (!force) {
            return {
              success: false,
              changed: false,
              error: `Existing .vscode/mcp.json is malformed: ${err.message}. Use force to overwrite.`,
            };
          }
          parsed = {};
        }
      }

      if (!parsed.servers) {
        parsed.servers = {};
      }

      const existingVsCode = parsed.servers['engram'];
      const vsCodeNeedsUpdate =
        !existingVsCode ||
        existingVsCode.command !== command ||
        JSON.stringify(existingVsCode.args) !== JSON.stringify(vsCodeServerBlock.args) ||
        existingVsCode.type !== 'stdio';

      if (vsCodeNeedsUpdate) {
        parsed.servers['engram'] = vsCodeServerBlock;
        this.writeAtomic(configPath, parsed);
        changed = true;
      }

      // 2. Write/update root .mcp.json (Copilot Agent Harness portable "mcpServers" schema)
      let rootParsed: McpConfigFile = {};
      if (fs.existsSync(rootConfigPath)) {
        try {
          const text = fs.readFileSync(rootConfigPath, 'utf8');
          rootParsed = this.parseJsonWithComments(text);
          if (!rootParsed || typeof rootParsed !== 'object') {
            rootParsed = {};
          }
        } catch (err: any) {
          if (force) {
            rootParsed = {};
          }
        }
      }

      if (!rootParsed.mcpServers) {
        rootParsed.mcpServers = {};
      }

      const existingRoot = rootParsed.mcpServers['engram'];
      const rootNeedsUpdate =
        !existingRoot ||
        existingRoot.command !== command ||
        JSON.stringify(existingRoot.args) !== JSON.stringify(rootServerBlock.args);

      if (rootNeedsUpdate) {
        rootParsed.mcpServers['engram'] = rootServerBlock;
        this.writeAtomic(rootConfigPath, rootParsed);
        changed = true;
      }

      return { success: true, changed };
    } catch (err: any) {
      return {
        success: false,
        changed: false,
        error: `Failed to write MCP configuration: ${err.message || String(err)}`,
      };
    }
  }

  /**
   * Surgically removes the Engram server from .vscode/mcp.json and root .mcp.json.
   */
  public remove(workspaceRoot: string): { success: boolean; removed: boolean; error?: string } {
    const configPath = this.getConfigPath(workspaceRoot);
    const rootConfigPath = this.getRootConfigPath(workspaceRoot);
    let removed = false;

    // Remove from .vscode/mcp.json
    if (fs.existsSync(configPath)) {
      try {
        const text = fs.readFileSync(configPath, 'utf8');
        const parsed: McpConfigFile = this.parseJsonWithComments(text);
        let fileRemoved = false;

        if (parsed.servers && parsed.servers['engram']) {
          delete parsed.servers['engram'];
          fileRemoved = true;
        }
        if (parsed.mcpServers && parsed.mcpServers['engram']) {
          delete parsed.mcpServers['engram'];
          fileRemoved = true;
        }

        if (fileRemoved) {
          this.writeAtomic(configPath, parsed);
          removed = true;
        }
      } catch (err: any) {
        return {
          success: false,
          removed: false,
          error: `Failed to update .vscode/mcp.json: ${err.message || String(err)}`,
        };
      }
    }

    // Remove from root .mcp.json
    if (fs.existsSync(rootConfigPath)) {
      try {
        const text = fs.readFileSync(rootConfigPath, 'utf8');
        const parsed: McpConfigFile = this.parseJsonWithComments(text);
        let fileRemoved = false;

        if (parsed.servers && parsed.servers['engram']) {
          delete parsed.servers['engram'];
          fileRemoved = true;
        }
        if (parsed.mcpServers && parsed.mcpServers['engram']) {
          delete parsed.mcpServers['engram'];
          fileRemoved = true;
        }

        if (fileRemoved) {
          this.writeAtomic(rootConfigPath, parsed);
          removed = true;
        }
      } catch (err: any) {
        return {
          success: false,
          removed: false,
          error: `Failed to update root .mcp.json: ${err.message || String(err)}`,
        };
      }
    }

    return { success: true, removed };
  }

  private writeAtomic(filePath: string, data: any): void {
    const tempPath = `${filePath}.tmp_${Date.now()}`;
    const content = JSON.stringify(data, null, 2) + '\n';
    fs.writeFileSync(tempPath, content, 'utf8');
    fs.renameSync(tempPath, filePath);
  }
}
