// Real-time syntax validation for the rRules DSL

import { lintProvider, Diagnostic } from '@codemirror/lint';
import { EditorState, Text } from '@codemirror/state';

/**
 * Syntax error interface
 */
export interface SyntaxError {
  position: { line: number; column: number };
  message: string;
  severity: 'error' | 'warning';
}

/**
 * Create a rule linter for CodeMirror 6
 */
export function createRuleLinter() {
  return lintProvider({
    async lint(state: EditorState) {
      const diagnostics: Diagnostic[] = [];
      const doc = state.doc;

      // Check line by line for common syntax issues
      for (let lineNum = 1; lineNum <= doc.lines; lineNum++) {
        const line = doc.line(lineNum);
        const text = line.text.trim();

        // Skip empty lines and comments
        if (!text || text.startsWith('#') || text.startsWith('//')) {
          continue;
        }

        // Check for IF without THEN
        if (text.toUpperCase().startsWith('IF')) {
          const hasThen = doc.toString().toUpperCase().includes(text + ' THEN')
            || text.includes('\nTHEN');
          if (!hasThen) {
            diagnostics.push({
              from: line.from,
              to: line.to,
              message: 'Missing THEN clause after IF',
              severity: 'warning',
            });
          }
        }

        // Check for unmatched braces in placeholders
        const openBraces = (text.match(/\{/g) || []).length;
        const closeBraces = (text.match(/\}/g) || []).length;
        if (openBraces !== closeBraces) {
          diagnostics.push({
            from: line.from,
            to: line.to,
            message: 'Unmatched braces in placeholder syntax',
            severity: 'error',
          });
        }

        // Check for common operator placement issues
        const operators = ['>', '<', '==', '!=', '>=', '<='];
        for (const op of operators) {
          if (text.includes(op)) {
            const parts = text.split(op);
            if (parts.length === 2) {
              const left = parts[0].trim();
              const right = parts[1].trim();
              if (!left || !right) {
                diagnostics.push({
                  from: line.from,
                  to: line.to,
                  message: `Invalid operator usage with '${op}'`,
                  severity: 'error',
                });
              }
            }
          }
        }
      }

      return diagnostics;
    },
  });
}

/**
 * Validate a rule document
 */
export function validateRuleDocument(content: string): SyntaxError[] {
  const errors: SyntaxError[] = [];
  const lines = content.split('\n');

  for (let i = 0; i < lines.length; i++) {
    const lineNum = i + 1;
    const text = lines[i].trim();

    if (!text || text.startsWith('#') || text.startsWith('//')) {
      continue;
    }

    // Check IF-THEN pairs
    const upperText = text.toUpperCase();
    if (upperText.startsWith('IF') && !upperText.includes(' THEN ')) {
      errors.push({
        position: { line: lineNum, column: 1 },
        message: 'Missing THEN clause after IF',
        severity: 'warning',
      });
    }

    // Check placeholder braces
    const openBraces = (text.match(/\{/g) || []).length;
    const closeBraces = (text.match(/\}/g) || []).length;
    if (openBraces !== closeBraces) {
      errors.push({
        position: { line: lineNum, column: text.indexOf('{') + 1 },
        message: 'Unmatched braces in placeholder syntax',
        severity: 'error',
      });
    }

    // Check for unknown keywords
    const words = text.split(/\s+/);
    for (const word of words) {
      const cleanWord = word.replace(/[{}[\](),;:]/g, '');
      if (cleanWord && !/\d/.test(cleanWord) && !cleanWord.startsWith('{')) {
        // Simple check - could be expanded
      }
    }
  }

  return errors;
}
