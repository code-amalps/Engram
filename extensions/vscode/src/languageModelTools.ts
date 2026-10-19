import * as vscode from 'vscode';
import { BinaryResolver } from './binaryResolver';
import { McpClient } from './mcpClient';
import { EngramOutputChannel } from './outputChannel';

export interface ToolDefinition {
  name: string;
  displayName: string;
  modelDescription: string;
  userDescription?: string;
  toolReferenceName?: string;
  canBeReferencedInPrompt?: boolean;
  icon?: string;
  inputSchema: any;
}

export const GOVERNED_TOOLS: ToolDefinition[] = [
  {
    name: 'engram_get_context',
    displayName: 'Engram: Compile Context',
    modelDescription:
      'Retrieve relevant project constraints, architecture rules, and token-budgeted context before implementing features or fixing non-trivial bugs. Call automatically when starting implementation tasks.',
    userDescription:
      'Compile token-optimized project context and engineering memory before editing.',
    toolReferenceName: 'engramContext',
    canBeReferencedInPrompt: true,
    icon: '$(sparkle)',
    inputSchema: {
      type: 'object',
      properties: {
        task: { type: 'string', description: 'The coding task goal, intent, or keywords' },
        token_budget: {
          type: 'integer',
          minimum: 1,
          description: 'Maximum token budget limit (default 4000)',
        },
      },
      required: ['task'],
    },
  },
  {
    name: 'engram_preflight',
    displayName: 'Engram: Preflight Handshake',
    modelDescription:
      'Pre-flight handshake: inspect blast radius, review governing decisions, and lock a Change Contract before high-risk or architectural modifications.',
    userDescription:
      'Lock a Change Contract and inspect blast radius before modifying code.',
    toolReferenceName: 'engramPreflight',
    canBeReferencedInPrompt: true,
    icon: '$(shield)',
    inputSchema: {
      type: 'object',
      properties: {
        task: { type: 'string', description: 'The coding task goal or intent' },
        token_budget: {
          type: 'integer',
          description: 'Context token budget limit (default 1200)',
        },
        forbidden: {
          type: 'array',
          items: { type: 'string' },
          description: 'Optional forbidden boundary patterns',
        },
      },
      required: ['task'],
    },
  },
  {
    name: 'engram_postflight',
    displayName: 'Engram: Postflight Audit',
    modelDescription:
      'Post-flight compliance audit: verify changes against Change Contract boundary rules and detect violations after completing modifications.',
    userDescription:
      'Verify changes against Change Contract boundary rules and detect violations.',
    toolReferenceName: 'engramPostflight',
    canBeReferencedInPrompt: true,
    icon: '$(pass)',
    inputSchema: {
      type: 'object',
      properties: {
        contract_id: {
          type: 'string',
          description: 'Active Change Contract ID to validate against',
        },
        strict: { type: 'boolean', description: 'Fail if unexpected files are modified' },
        staged: {
          type: 'boolean',
          description: 'Validate git staged index instead of entire working tree',
        },
      },
    },
  },
  {
    name: 'engram_get_impact',
    displayName: 'Engram: Get Change Impact',
    modelDescription:
      'Analyze reverse dependencies and blast radius for a file or symbol. Call automatically before modifying shared services, public interfaces, state, or routing.',
    userDescription: 'Fast 1-hop reverse dependency and blast radius impact query.',
    toolReferenceName: 'engramImpact',
    canBeReferencedInPrompt: true,
    icon: '$(references)',
    inputSchema: {
      type: 'object',
      properties: {
        target: { type: 'string', description: 'Symbol name or relative file path' },
      },
      required: ['target'],
    },
  },
  {
    name: 'engram_inspect_symbol',
    displayName: 'Engram: Inspect Symbol',
    modelDescription:
      'Multi-hop blast radius impact query, Change Contract authorization check, and scoped memory lookup for a target symbol or file.',
    userDescription: 'Multi-hop blast radius impact query and scoped memory lookup.',
    toolReferenceName: 'engramInspectSymbol',
    canBeReferencedInPrompt: true,
    icon: '$(search)',
    inputSchema: {
      type: 'object',
      properties: {
        target: { type: 'string', description: 'Target symbol or file path to inspect' },
        depth: {
          type: 'integer',
          minimum: 1,
          maximum: 3,
          description: 'Dependency traversal depth (default 2)',
        },
      },
      required: ['target'],
    },
  },
  {
    name: 'engram_why',
    displayName: 'Engram: Why (Rationale)',
    modelDescription:
      'Explain architectural rationale, governing decisions, and commit history for a symbol or file. Call automatically when investigating why code was structured this way or why tests fail.',
    userDescription:
      'Architectural archaeology: explain rationale and governing decisions.',
    toolReferenceName: 'engramWhy',
    canBeReferencedInPrompt: true,
    icon: '$(history)',
    inputSchema: {
      type: 'object',
      properties: {
        target: { type: 'string', description: 'Symbol name or relative file path' },
      },
      required: ['target'],
    },
  },
  {
    name: 'engram_get_cochanges',
    displayName: 'Engram: Get Co-changes',
    modelDescription:
      'Retrieve historical Git co-change coupling signals and statistics for a given file.',
    userDescription:
      'Retrieve historical Git co-change coupling signals and statistics.',
    toolReferenceName: 'engramCochanges',
    canBeReferencedInPrompt: true,
    icon: '$(git-commit)',
    inputSchema: {
      type: 'object',
      properties: {
        file_path: { type: 'string', description: 'Relative file path within repository' },
        min_probability: {
          type: 'number',
          minimum: 0.0,
          maximum: 1.0,
          description: 'Minimum co-change probability threshold',
        },
      },
      required: ['file_path'],
    },
  },
  {
    name: 'engram_get_memory',
    displayName: 'Engram: Get Memory',
    modelDescription:
      'Retrieve durable architectural decisions, policies, and institutional engineering memories.',
    userDescription:
      'Retrieve durable architectural decisions, policies, and institutional engineering memories.',
    toolReferenceName: 'engramMemory',
    canBeReferencedInPrompt: true,
    icon: '$(bookmark)',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Semantic query filter' },
        files: {
          type: 'array',
          items: { type: 'string' },
          description: 'Scope files to match',
        },
      },
    },
  },
  {
    name: 'engram_record_decision',
    displayName: 'Engram: Record Decision',
    modelDescription:
      'Propose an architectural decision into engineering memory (marked PROVISIONAL until human ratification).',
    userDescription: 'Propose an architectural decision into engineering memory.',
    toolReferenceName: 'engramRecordDecision',
    canBeReferencedInPrompt: true,
    icon: '$(edit)',
    inputSchema: {
      type: 'object',
      properties: {
        title: { type: 'string', description: 'Short title of the architectural decision' },
        statement: {
          type: 'string',
          description: 'The exact decision policy or architectural invariant'
        },
        reason: { type: 'string', description: 'Underlying rationale or trade-off explanation' },
        modules: {
          type: 'array',
          items: { type: 'string' },
          description: 'Architectural modules governed by this decision',
        },
        files: {
          type: 'array',
          items: { type: 'string' },
          description: 'Specific files governed by this decision',
        },
        accepted_by: {
          type: 'string',
          description: 'Agent or engineer proposing this decision',
        },
      },
      required: ['title', 'statement', 'reason'],
    },
  },
  {
    name: 'engram_check_constitution',
    displayName: 'Engram: Check Constitution',
    modelDescription:
      'Evaluate architectural constitution rules and boundary isolation policies against changed files.',
    userDescription:
      'Evaluate architectural constitution rules against modified files.',
    toolReferenceName: 'engramConstitution',
    canBeReferencedInPrompt: true,
    icon: '$(law)',
    inputSchema: {
      type: 'object',
      properties: {
        files: {
          type: 'array',
          items: { type: 'string' },
          description: 'Specific file paths to check (defaults to git diff)',
        },
      },
    },
  },
  {
    name: 'engram_get_ecosystem',
    displayName: 'Engram: Get Ecosystem',
    modelDescription:
      'Detect workspace ecosystem, primary language, manifest dependencies, build tools, and framework rules.',
    userDescription:
      'Detect workspace ecosystem, language, dependencies, and build tools.',
    toolReferenceName: 'engramEcosystem',
    canBeReferencedInPrompt: true,
    icon: '$(package)',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'engram_doctor',
    displayName: 'Engram: Doctor',
    modelDescription:
      'Run diagnostic health checks on memory storage, index integrity, and append-only cryptographic chains.',
    userDescription:
      'Run diagnostic health checks on memory storage and index integrity.',
    toolReferenceName: 'engramDoctor',
    canBeReferencedInPrompt: true,
    icon: '$(pulse)',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'engram_recover',
    displayName: 'Engram: Recover Contract',
    modelDescription:
      'Formulate structured, non-destructive recovery recipe for contract violations and check exhaustion gate.',
    userDescription:
      'Formulate structured recovery recipe for contract violations.',
    toolReferenceName: 'engramRecover',
    canBeReferencedInPrompt: true,
    icon: '$(debug-restart)',
    inputSchema: {
      type: 'object',
      properties: {
        contract_id: {
          type: 'string',
          description: 'Change Contract ID to formulate recovery plan for',
        },
      },
    },
  },
  {
    name: 'engram_drift',
    displayName: 'Engram: Drift Analysis',
    modelDescription:
      'Analyze architectural decision drift, boundary erosion, and scoped staleness across commits.',
    userDescription:
      'Analyze architectural decision drift and staleness across commits.',
    toolReferenceName: 'engramDrift',
    canBeReferencedInPrompt: true,
    icon: '$(git-compare)',
    inputSchema: {
      type: 'object',
      properties: {
        module: { type: 'string', description: 'Filter by module name' },
        decision: { type: 'string', description: 'Filter by decision ID' },
        since: { type: 'string', description: 'Analyze commits since this commit hash' },
        staleness_threshold: {
          type: 'integer',
          description: 'Configured mutation threshold for STALE_REVIEW (default 5)',
        },
      },
    },
  },
  {
    name: 'engram_ratify_decision',
    displayName: 'Engram: Ratify Decision',
    modelDescription:
      'Explicitly ratify an architectural decision with human reviewer authorization and provenance tracking.',
    userDescription:
      'Explicitly ratify an architectural decision with reviewer authorization.',
    toolReferenceName: 'engramRatify',
    canBeReferencedInPrompt: true,
    icon: '$(verified)',
    inputSchema: {
      type: 'object',
      properties: {
        decision_id: { type: 'string', description: 'The decision ID to ratify' },
        by: { type: 'string', description: 'Reviewer identity or authorizing human' },
        reason: { type: 'string', description: 'Ratification rationale or justification' },
        commit: {
          type: 'string',
          description: 'Git commit SHA being ratified (defaults to HEAD)',
        },
      },
      required: ['decision_id'],
    },
  },
];

export class EngramLanguageModelToolProvider {
  constructor(
    private binaryResolver: BinaryResolver,
    private mcpClient: McpClient,
    private outputChannel?: EngramOutputChannel
  ) {}

  public registerAll(
    context: vscode.ExtensionContext,
    getWorkspaceRoot: () => string | undefined
  ): vscode.Disposable[] {
    const disposables: vscode.Disposable[] = [];

    if (typeof (vscode as any).lm?.registerTool !== 'function') {
      this.outputChannel?.log(
        'vscode.lm.registerTool is not available in this VS Code environment; skipping Language Model Tools registration.',
        'WARN'
      );
      return disposables;
    }

    for (const toolDef of GOVERNED_TOOLS) {
      try {
        const disposable = (vscode as any).lm.registerTool(toolDef.name, {
          invoke: async (
            options: vscode.LanguageModelToolInvocationOptions<any>,
            token: vscode.CancellationToken
          ): Promise<vscode.LanguageModelToolResult> => {
            return this.invokeTool(toolDef.name, options, token, getWorkspaceRoot);
          },
          prepareInvocation: (
            _options: vscode.LanguageModelToolInvocationPrepareOptions<any>,
            _token: vscode.CancellationToken
          ): vscode.PreparedToolInvocation => {
            return {
              invocationMessage: `Invoking Engram ${toolDef.name}...`,
            };
          },
        });
        disposables.push(disposable);
        context.subscriptions.push(disposable);
      } catch (err: any) {
        this.outputChannel?.log(
          `Failed to register tool ${toolDef.name}: ${err?.message || err}`,
          'ERROR'
        );
      }
    }

    this.outputChannel?.log(
      `Registered ${disposables.length} Engram tools into VS Code Language Model Tool catalog.`
    );
    return disposables;
  }

  public async invokeTool(
    toolName: string,
    options: vscode.LanguageModelToolInvocationOptions<any>,
    _token: vscode.CancellationToken,
    getWorkspaceRoot: () => string | undefined
  ): Promise<vscode.LanguageModelToolResult> {
    const wsRoot = getWorkspaceRoot();
    if (!wsRoot) {
      return this.formatResult(
        `Error: No active workspace folder open. Open a folder to use ${toolName}.`
      );
    }

    const cfg = vscode.workspace.getConfiguration('engram');
    const exp = cfg.get<string>('binaryPath', '');
    const timeoutMs = cfg.get<number>('timeoutMs', 30000);

    const resolution = await this.binaryResolver.resolve(exp, wsRoot);
    if (!resolution.valid) {
      return this.formatResult(
        `Error: Engram binary not found (${resolution.error || 'unknown error'}). ` +
          `Please ensure Engram CLI is installed or configure 'engram.binaryPath' in settings.`
      );
    }

    this.outputChannel?.log(
      `Invoking LanguageModelTool: ${toolName} with args: ${JSON.stringify(options?.input || {})}`
    );

    try {
      const res = await this.mcpClient.callTool(
        resolution.path,
        wsRoot,
        toolName,
        options?.input || {},
        timeoutMs
      );

      if (!res.success) {
        this.outputChannel?.log(`Tool ${toolName} failed: ${res.error}`, 'ERROR');
        return this.formatResult(`Error invoking ${toolName}: ${res.error}`);
      }

      // Format response according to MCP text content
      const content = res.result?.content;
      let text = '';
      if (Array.isArray(content)) {
        text = content
          .map((part: any) => {
            if (part && part.type === 'text' && typeof part.text === 'string') {
              return part.text;
            }
            return JSON.stringify(part);
          })
          .join('\n');
      } else if (typeof res.result === 'string') {
        text = res.result;
      } else {
        text = JSON.stringify(res.result, null, 2);
      }

      this.outputChannel?.log(
        `Tool ${toolName} completed successfully (${text.length} chars).`
      );
      return this.formatResult(text);
    } catch (err: any) {
      this.outputChannel?.log(
        `Unhandled error invoking ${toolName}: ${err?.message || err}`,
        'ERROR'
      );
      return this.formatResult(`Error: ${err?.message || String(err)}`);
    }
  }

  public formatResult(text: string): vscode.LanguageModelToolResult {
    const ToolResultClass = (vscode as any).LanguageModelToolResult;
    const TextPartClass = (vscode as any).LanguageModelTextPart;
    if (typeof ToolResultClass === 'function' && typeof TextPartClass === 'function') {
      return new ToolResultClass([new TextPartClass(text)]);
    }
    // Fallback for mocked or older environments
    return {
      content: [{ type: 'text', value: text }],
    } as any;
  }
}
