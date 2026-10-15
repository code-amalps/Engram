import * as vscode from 'vscode';
import { DurableMemory, BinaryResolutionResult } from './types';
import { EngramClient } from './engramClient';

export class MemoryTreeItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly collapsibleState: vscode.TreeItemCollapsibleState,
    public readonly memory?: DurableMemory,
    public readonly isCategory = false,
    public readonly scopeCategory?: string,
    public readonly isStateItem = false
  ) {
    super(label, collapsibleState);

    if (isCategory) {
      this.iconPath = new vscode.ThemeIcon('folder');
      this.contextValue = 'memoryCategory';
    } else if (memory) {
      this.id = memory.id;
      const ratifiedInfo = memory.ratified_at_commit
        ? `\n\n**Ratified At:** \`${memory.ratified_at_commit}\``
        : '';
      const moduleInfo =
        memory.scope.modules.length > 0 ? ` (\`${memory.scope.modules.join(', ')}\`)` : '';

      this.tooltip = new vscode.MarkdownString(
        `### ${memory.title}\n\n` +
        `**Decision ID:** \`${memory.id}\`\n\n` +
        `**Statement:** ${memory.statement}\n\n` +
        `**Rationale:** ${memory.reason}\n\n` +
        `**Scope:** \`${memory.scope.level}\`${moduleInfo}\n\n` +
        `**Accepted By:** ${memory.accepted_by} (${new Date(memory.accepted_at).toLocaleString()})` +
        ratifiedInfo
      );
      this.description = memory.scope.level;
      this.iconPath = new vscode.ThemeIcon('bookmark', new vscode.ThemeColor('symbolIcon.keywordForeground'));
      this.contextValue = 'durableMemory';
      this.command = {
        command: 'engram.showMemoryDetails',
        title: 'Show Memory Details',
        arguments: [memory],
      };
    } else if (isStateItem) {
      this.iconPath = new vscode.ThemeIcon('info');
      this.contextValue = 'memoryState';
    }
  }
}

export class MemoryTreeProvider implements vscode.TreeDataProvider<MemoryTreeItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<MemoryTreeItem | undefined | null | void> =
    new vscode.EventEmitter<MemoryTreeItem | undefined | null | void>();
  public readonly onDidChangeTreeData: vscode.Event<MemoryTreeItem | undefined | null | void> =
    this._onDidChangeTreeData.event;

  private memories: DurableMemory[] = [];
  private binaryPath: string | null = null;
  private workspaceRoot: string | null = null;
  private lastError: string | null = null;
  private isLoading = false;

  constructor(private client: EngramClient) {}

  public setResolution(resolution: BinaryResolutionResult, workspaceRoot?: string): void {
    if (resolution.valid) {
      this.binaryPath = resolution.path;
      this.workspaceRoot = workspaceRoot || null;
      this.lastError = null;
    } else {
      this.binaryPath = null;
      this.workspaceRoot = null;
      this.lastError = resolution.error;
    }
    this.refresh();
  }

  public setWorkspaceRoot(workspaceRoot: string): void {
    this.workspaceRoot = workspaceRoot;
    this.refresh();
  }

  public refresh(): void {
    this._onDidChangeTreeData.fire();
  }

  public getTreeItem(element: MemoryTreeItem): vscode.TreeItem {
    return element;
  }

  public async getChildren(element?: MemoryTreeItem): Promise<MemoryTreeItem[]> {
    if (!this.binaryPath) {
      return [
        new MemoryTreeItem(
          this.lastError || 'Engram binary not detected. Click status bar to configure.',
          vscode.TreeItemCollapsibleState.None,
          undefined,
          false,
          undefined,
          true
        ),
      ];
    }

    if (!this.workspaceRoot) {
      return [
        new MemoryTreeItem(
          'Please open a workspace folder to view engineering memories.',
          vscode.TreeItemCollapsibleState.None,
          undefined,
          false,
          undefined,
          true
        ),
      ];
    }

    // Root level: fetch memories
    if (!element) {
      this.isLoading = true;
      try {
        this.memories = await this.client.listMemories(this.binaryPath, this.workspaceRoot);
        this.lastError = null;
      } catch (err: any) {
        this.lastError = err.message || String(err);
        return [
          new MemoryTreeItem(
            `Error loading memories: ${this.lastError}`,
            vscode.TreeItemCollapsibleState.None,
            undefined,
            false,
            undefined,
            true
          ),
        ];
      } finally {
        this.isLoading = false;
      }

      if (this.memories.length === 0) {
        return [
          new MemoryTreeItem(
            'No durable memories recorded (.engram/memory.yaml)',
            vscode.TreeItemCollapsibleState.None,
            undefined,
            false,
            undefined,
            true
          ),
        ];
      }

      // Group memories by scope level
      const scopes = ['GLOBAL', 'MODULE', 'SYMBOL', 'OTHER'];
      const scopeGroups = new Map<string, DurableMemory[]>();

      for (const m of this.memories) {
        const lvl = scopes.includes(m.scope.level.toUpperCase()) ? m.scope.level.toUpperCase() : 'OTHER';
        if (!scopeGroups.has(lvl)) {
          scopeGroups.set(lvl, []);
        }
        scopeGroups.get(lvl)!.push(m);
      }

      const items: MemoryTreeItem[] = [];
      for (const scope of scopes) {
        const group = scopeGroups.get(scope);
        if (group && group.length > 0) {
          items.push(
            new MemoryTreeItem(
              `${scope} Decisions (${group.length})`,
              vscode.TreeItemCollapsibleState.Expanded,
              undefined,
              true,
              scope
            )
          );
        }
      }
      return items;
    }

    // Category child level
    if (element.isCategory && element.scopeCategory) {
      const filtered = this.memories.filter((m) => {
        const lvl = m.scope.level.toUpperCase();
        if (element.scopeCategory === 'OTHER') {
          return !['GLOBAL', 'MODULE', 'SYMBOL'].includes(lvl);
        }
        return lvl === element.scopeCategory;
      });

      return filtered.map(
        (m) =>
          new MemoryTreeItem(
            m.title,
            vscode.TreeItemCollapsibleState.None,
            m
          )
      );
    }

    return [];
  }
}
