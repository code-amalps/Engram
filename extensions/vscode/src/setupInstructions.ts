import * as fs from 'fs';
import * as path from 'path';

export const START_MARKER = '<!-- BEGIN ENGRAM MANAGED BLOCK -->';
export const END_MARKER = '<!-- END ENGRAM MANAGED BLOCK -->';
export const LEGACY_START_MARKER = '<!-- ENGRAM_MANAGED_START -->';
export const LEGACY_END_MARKER = '<!-- ENGRAM_MANAGED_END -->';

export const DEFAULT_COPILOT_INSTRUCTION = `${START_MARKER}
## Engram Architecture Context & Task-Aware Intelligence
When working on tasks in this repository, automatically invoke the relevant Engram tools based on scope and risk:
- **Feature Implementation & Non-Trivial Bug Fixes**: Call \`engram_get_context\` with the task description before editing to retrieve architectural rules, boundaries, and conventions.
- **Modifying Shared Services, State, or APIs**: Call \`engram_get_impact\` on the target file or symbol before editing cross-cutting components to evaluate reverse dependencies and avoid breaking callers.
- **Architectural or High-Risk Modifications**: Call \`engram_preflight\` before editing to inspect blast radius, review governing decisions, and lock a Change Contract.
- **Post-Implementation Verification**: Call \`engram_postflight\` after modifying code under a Change Contract to audit compliance and record candidate memories.
- **Investigating Failures or Design Rationale**: Call \`engram_why\` to retrieve rationale, governing decisions, and commit archaeology.
- **Trivial / Leaf Edits**: Do not invoke tools for simple typos, comments, or isolated leaf UI text adjustments.
${END_MARKER}`;

export class SetupInstructionsManager {
  public getInstructionsPath(workspaceRoot: string): string {
    return path.join(workspaceRoot, '.github', 'copilot-instructions.md');
  }

  public syncInstructions(
    workspaceRoot: string,
    remove = false
  ): { success: boolean; changed: boolean; path: string; error?: string } {
    const filePath = this.getInstructionsPath(workspaceRoot);
    const githubDir = path.dirname(filePath);

    try {
      const exists = fs.existsSync(filePath);
      const existingText = exists ? fs.readFileSync(filePath, 'utf8') : '';

      let startIndex = existingText.indexOf(START_MARKER);
      let endIndex = existingText.indexOf(END_MARKER);
      let endMarkerLen = END_MARKER.length;

      if (startIndex === -1 || endIndex === -1) {
        // Fallback to legacy markers if present
        const legStart = existingText.indexOf(LEGACY_START_MARKER);
        const legEnd = existingText.indexOf(LEGACY_END_MARKER);
        if (legStart !== -1 && legEnd !== -1 && legEnd > legStart) {
          startIndex = legStart;
          endIndex = legEnd;
          endMarkerLen = LEGACY_END_MARKER.length;
        }
      }

      const hasBlock = startIndex !== -1 && endIndex !== -1 && endIndex > startIndex;

      if (remove) {
        if (!hasBlock) {
          return { success: true, changed: false, path: filePath };
        }
        // Strip block
        const before = existingText.substring(0, startIndex).trimEnd();
        const after = existingText.substring(endIndex + endMarkerLen).trimStart();
        const combined = [before, after].filter(Boolean).join('\n\n');

        if (combined.trim() === '') {
          fs.unlinkSync(filePath);
        } else {
          fs.writeFileSync(filePath, combined + '\n', 'utf8');
        }
        return { success: true, changed: true, path: filePath };
      }

      // Add or update managed block
      if (hasBlock) {
        const currentBlock = existingText.substring(startIndex, endIndex + endMarkerLen);
        if (currentBlock.trim() === DEFAULT_COPILOT_INSTRUCTION.trim()) {
          return { success: true, changed: false, path: filePath };
        }
        const before = existingText.substring(0, startIndex);
        const after = existingText.substring(endIndex + endMarkerLen);
        const updated = before + DEFAULT_COPILOT_INSTRUCTION + after;
        fs.writeFileSync(filePath, updated, 'utf8');
        return { success: true, changed: true, path: filePath };
      }

      // Block not present: append
      if (!fs.existsSync(githubDir)) {
        fs.mkdirSync(githubDir, { recursive: true });
      }

      const newContent = existingText.trim() === ''
        ? DEFAULT_COPILOT_INSTRUCTION + '\n'
        : existingText.trimEnd() + '\n\n' + DEFAULT_COPILOT_INSTRUCTION + '\n';

      fs.writeFileSync(filePath, newContent, 'utf8');
      return { success: true, changed: true, path: filePath };
    } catch (err: any) {
      return {
        success: false,
        changed: false,
        path: filePath,
        error: `Failed to update instructions: ${err.message || String(err)}`,
      };
    }
  }
}
