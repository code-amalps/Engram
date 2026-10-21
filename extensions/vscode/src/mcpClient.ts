import * as cp from 'child_process';
import * as readline from 'readline';
import { McpHealthCheckResult } from './types';

export class McpClient {
  /**
   * Spawns the Engram MCP server over stdio, executes the protocol handshake,
   * verifies tool discovery, and measures latency.
   */
  public async checkHealth(
    executablePath: string,
    workspaceRoot: string,
    timeoutMs = 5000
  ): Promise<McpHealthCheckResult> {
    const startTime = Date.now();
    let child: cp.ChildProcess | null = null;
    let rl: readline.Interface | null = null;
    let timeoutTimer: NodeJS.Timeout | null = null;
    let isTerminated = false;

    const terminate = () => {
      if (isTerminated) {
        return;
      }
      isTerminated = true;

      if (timeoutTimer) {
        clearTimeout(timeoutTimer);
        timeoutTimer = null;
      }
      if (rl) {
        try {
          rl.close();
        } catch {
          // ignore
        }
        rl = null;
      }
      if (child && child.pid) {
        try {
          if (process.platform === 'win32') {
            try {
              child.kill();
            } catch {
              // ignore
            }
            try {
              cp.spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], {
                windowsHide: true,
                stdio: 'ignore',
              });
            } catch {
              // ignore
            }
          } else {
            child.kill('SIGTERM');
          }
        } catch {
          // Process may have already exited
        }
        child = null;
      }
    };

    return new Promise<McpHealthCheckResult>((resolve) => {
      timeoutTimer = setTimeout(() => {
        terminate();
        resolve({
          success: false,
          latencyMs: Date.now() - startTime,
          toolCount: 0,
          tools: [],
          protocolVersion: '',
          serverName: '',
          error: `MCP handshake timed out after ${timeoutMs}ms.`,
        });
      }, timeoutMs);

      try {
        child = cp.spawn(executablePath, ['-C', workspaceRoot, 'mcp'], {
          shell: false,
          windowsHide: true,
          stdio: ['pipe', 'pipe', 'ignore'],
        });

        if (!child.stdin || !child.stdout) {
          terminate();
          resolve({
            success: false,
            latencyMs: Date.now() - startTime,
            toolCount: 0,
            tools: [],
            protocolVersion: '',
            serverName: '',
            error: 'Failed to open stdio pipes for Engram MCP server.',
          });
          return;
        }

        const stdin = child.stdin;
        rl = readline.createInterface({
          input: child.stdout,
          crlfDelay: Infinity,
        });

        child.on('error', (err) => {
          terminate();
          resolve({
            success: false,
            latencyMs: Date.now() - startTime,
            toolCount: 0,
            tools: [],
            protocolVersion: '',
            serverName: '',
            error: `Failed to spawn MCP process: ${err.message}`,
          });
        });

        child.on('exit', (code) => {
          if (!isTerminated) {
            terminate();
            resolve({
              success: false,
              latencyMs: Date.now() - startTime,
              toolCount: 0,
              tools: [],
              protocolVersion: '',
              serverName: '',
              error: `MCP process exited unexpectedly with code ${code}.`,
            });
          }
        });

        let phase: 'INITIALIZE' | 'TOOLS_LIST' = 'INITIALIZE';
        let protocolVersion = '';
        let serverName = '';

        rl.on('line', (line) => {
          const trimmed = line.trim();
          if (!trimmed) {
            return;
          }

          let msg: any;
          try {
            msg = JSON.parse(trimmed);
          } catch {
            return; // Ignore non-JSON lines if any
          }

          if (msg.error) {
            terminate();
            resolve({
              success: false,
              latencyMs: Date.now() - startTime,
              toolCount: 0,
              tools: [],
              protocolVersion,
              serverName,
              error: `MCP RPC error: ${JSON.stringify(msg.error)}`,
            });
            return;
          }

          if (phase === 'INITIALIZE' && msg.id === 1) {
            const res = msg.result;
            if (!res) {
              terminate();
              resolve({
                success: false,
                latencyMs: Date.now() - startTime,
                toolCount: 0,
                tools: [],
                protocolVersion: '',
                serverName: '',
                error: 'MCP initialize response missing result object.',
              });
              return;
            }

            serverName = res.serverInfo?.name || '';
            protocolVersion = res.protocolVersion || '';

            if (serverName !== 'engram') {
              terminate();
              resolve({
                success: false,
                latencyMs: Date.now() - startTime,
                toolCount: 0,
                tools: [],
                protocolVersion,
                serverName,
                error: `Server identity mismatch: expected 'engram', got '${serverName}'.`,
              });
              return;
            }

            // Step 2: Request tools/list
            phase = 'TOOLS_LIST';
            const toolsReq = {
              jsonrpc: '2.0',
              id: 2,
              method: 'tools/list',
              params: {},
            };
            stdin.write(JSON.stringify(toolsReq) + '\n');
            return;
          }

          if (phase === 'TOOLS_LIST' && msg.id === 2) {
            const res = msg.result;
            const tools = Array.isArray(res?.tools) ? res.tools.map((t: any) => String(t.name)) : [];
            const uniqueTools = new Set(tools);

            terminate();
            const latencyMs = Date.now() - startTime;

            if (tools.length === 0) {
              resolve({
                success: false,
                latencyMs,
                toolCount: 0,
                tools: [],
                protocolVersion,
                serverName,
                error: 'Engram MCP server returned 0 tools.',
              });
              return;
            }

            if (uniqueTools.size !== tools.length) {
              resolve({
                success: false,
                latencyMs,
                toolCount: tools.length,
                tools,
                protocolVersion,
                serverName,
                error: 'Duplicate tool definitions discovered from MCP server.',
              });
              return;
            }

            resolve({
              success: true,
              latencyMs,
              toolCount: tools.length,
              tools,
              protocolVersion,
              serverName,
            });
          }
        });

        // Step 1: Send initialize request
        const initReq = {
          jsonrpc: '2.0',
          id: 1,
          method: 'initialize',
          params: {
            protocolVersion: '2024-11-05',
            capabilities: {},
            clientInfo: {
              name: 'engram-vscode-extension',
              version: '1.5.2',
            },
          },
        };
        stdin.write(JSON.stringify(initReq) + '\n');
      } catch (err: any) {
        terminate();
        resolve({
          success: false,
          latencyMs: Date.now() - startTime,
          toolCount: 0,
          tools: [],
          protocolVersion: '',
          serverName: '',
          error: `Handshake execution failed: ${err.message || String(err)}`,
        });
      }
    });
  }

  /**
   * Invokes a read-only tool over MCP to verify end-to-end execution.
   */
  public async callTool(
    executablePath: string,
    workspaceRoot: string,
    toolName: string,
    args: Record<string, any> = {},
    timeoutMs = 5000
  ): Promise<{ success: boolean; result?: any; error?: string }> {
    return new Promise((resolve) => {
      let child: cp.ChildProcess | null = null;
      let rl: readline.Interface | null = null;
      let timeoutTimer: NodeJS.Timeout | null = null;
      let isTerminated = false;

      const terminate = () => {
        if (isTerminated) {
          return;
        }
        isTerminated = true;
        if (timeoutTimer) {
          clearTimeout(timeoutTimer);
          timeoutTimer = null;
        }
        if (rl) {
          try {
            rl.close();
          } catch {
            // ignore
          }
          rl = null;
        }
        if (child && child.pid) {
          try {
            child.kill();
          } catch {
            // ignore
          }
          child = null;
        }
      };

      timeoutTimer = setTimeout(() => {
        terminate();
        resolve({ success: false, error: `Tool call '${toolName}' timed out.` });
      }, timeoutMs);

      try {
        child = cp.spawn(executablePath, ['-C', workspaceRoot, 'mcp'], {
          shell: false,
          windowsHide: true,
          stdio: ['pipe', 'pipe', 'ignore'],
        });

        if (!child.stdin || !child.stdout) {
          terminate();
          resolve({ success: false, error: 'Failed to open stdio pipes.' });
          return;
        }

        const stdin = child.stdin;
        rl = readline.createInterface({
          input: child.stdout,
          crlfDelay: Infinity,
        });

        let phase: 'INITIALIZE' | 'CALL_TOOL' = 'INITIALIZE';

        rl.on('line', (line) => {
          const trimmed = line.trim();
          if (!trimmed) {
            return;
          }
          let msg: any;
          try {
            msg = JSON.parse(trimmed);
          } catch {
            return;
          }

          if (phase === 'INITIALIZE' && msg.id === 1) {
            phase = 'CALL_TOOL';
            const callReq = {
              jsonrpc: '2.0',
              id: 2,
              method: 'tools/call',
              params: {
                name: toolName,
                arguments: args,
              },
            };
            stdin.write(JSON.stringify(callReq) + '\n');
            return;
          }

          if (phase === 'CALL_TOOL' && msg.id === 2) {
            terminate();
            if (msg.error) {
              resolve({ success: false, error: JSON.stringify(msg.error) });
            } else {
              resolve({ success: true, result: msg.result });
            }
          }
        });

        // Initialize first
        const initReq = {
          jsonrpc: '2.0',
          id: 1,
          method: 'initialize',
          params: {
            protocolVersion: '2024-11-05',
            capabilities: {},
            clientInfo: { name: 'engram-vscode-extension', version: '1.5.2' },
          },
        };
        stdin.write(JSON.stringify(initReq) + '\n');
      } catch (err: any) {
        terminate();
        resolve({ success: false, error: err.message || String(err) });
      }
    });
  }
}
