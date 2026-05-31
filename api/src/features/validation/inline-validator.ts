// Inline validation engine with comprehensive syntax and semantic checks

import { Diagnostic } from '@codemirror/lint';
import { EditorState, Text } from '@codemirror/state';

/**
 * Validation result structure
 */
export interface ValidationResult {
  errors: ValidationError[];
  warnings: ValidationWarning[];
  infos: ValidationInfo[];
}

/**
 * Error with position data for inline display
 */
export interface ValidationError {
  message: string;
  position: { line: number; column: number };
  severity: 'error';
  hint?: string;
}

/**
 * Warning with position data
 */
export interface ValidationWarning {
  message: string;
  position: { line: number; column: number };
  severity: 'warning';
  hint?: string;
}

/**
 * Info message with position
 */
export interface ValidationInfo {
  message: string;
  position: { line: number; column: number };
  severity: 'info';
}

/**
 * Entity schema field definition
 */
export interface SchemaField {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'date' | 'enum';
  options?: string[];
  required?: boolean;
}

/**
 * Entity schema definition
 */
export interface EntitySchema {
  name: string;
  fields: SchemaField[];
}

/**
 * Rule structure for semantic validation
 */
export interface RuleStructure {
  conditions: Condition[];
  actions: Action[];
}

export interface Condition {
  field: string;
  operator: string;
  value: any;
  type?: 'and' | 'or';
}

export interface Action {
  type: string;
  parameters: Record<string, any>;
}

/**
 * Known operators for validation
 */
const KNOWN_OPERATORS = ['==', '!=', '>', '<', '>=', '<=', 'contains', 'startsWith', 'endsWith'];

/**
 * Known action types
 */
const KNOWN_ACTIONS = [
  'apply_discount',
  'add_tag',
  'send_email',
  'create_task',
  'update_field',
  'notify_webhook',
];

/**
 * SemanticValidator - Validates rule semantics against schema
 */
export class SemanticValidator {
  private schema: EntitySchema | null;

  constructor(schema: EntitySchema | null = null) {
    this.schema = schema;
  }

  /**
   * Validate the entire rule structure
   */
  validate(ruleStructure: RuleStructure): ValidationResult {
    const errors: ValidationError[] = [];
    const warnings: ValidationWarning[] = [];

    // Check conditions for field and type validity
    for (const condition of ruleStructure.conditions) {
      const fieldError = this.validateFieldReference(condition.field);
      if (fieldError) {
        errors.push(fieldError);
      }

      const typeError = this.validateFieldType(condition.field, condition.value);
      if (typeError) {
        warnings.push(typeError);
      }
    }

    // Check actions for validity
    for (const action of ruleStructure.actions) {
      const actionError = this.validateAction(action.type, action.parameters);
      if (actionError) {
        errors.push(actionError);
      }
    }

    return { errors, warnings, infos: [] };
  }

  /**
   * Validate that a field reference exists in the schema
   */
  validateFieldReference(field: string): ValidationError | null {
    if (!this.schema) {
      return null; // No schema to validate against
    }

    const existingField = this.schema.fields.find((f) => f.name === field);
    if (!existingField) {
      // Suggest similar fields
      const suggestions = this.suggestField(field);
      let hint = '';
      if (suggestions.length > 0) {
        hint = `Did you mean: ${suggestions.slice(0, 3).join(', ')}?`;
      }
      return {
        message: `Unknown field reference: '${field}'`,
        position: { line: 1, column: 1 },
        severity: 'error',
        hint,
      };
    }

    return null;
  }

  /**
   * Validate that a value matches the expected type for a field
   */
  validateFieldType(field: string, value: any): ValidationWarning | null {
    if (!this.schema) {
      return null; // No schema to validate against
    }

    const fieldDef = this.schema.fields.find((f) => f.name === field);
    if (!fieldDef) {
      return null; // Skip type check for unknown fields
    }

    const valueStr = String(value);
    let isCorrectType = true;
    let expectedType = '';

    switch (fieldDef.type) {
      case 'number':
        expectedType = 'number';
        isCorrectType = !isNaN(parseFloat(valueStr)) && isFinite(parseFloat(valueStr));
        break;
      case 'boolean':
        expectedType = 'boolean';
        isCorrectType =
          valueStr === 'true' || valueStr === 'false' || valueStr === '1' || valueStr === '0';
        break;
      case 'date':
        expectedType = 'date (YYYY-MM-DD)';
        const dateValue = new Date(valueStr);
        isCorrectType = !isNaN(dateValue.getTime());
        break;
      case 'enum':
        if (fieldDef.options) {
          expectedType = `one of: ${fieldDef.options.join(', ')}`;
          isCorrectType = fieldDef.options.includes(valueStr);
        }
        break;
      // string type passes for any value
    }

    if (!isCorrectType) {
      return {
        message: `Value '${value}' does not match expected type ${expectedType} for field '${field}'`,
        position: { line: 1, column: 1 },
        severity: 'warning',
        hint: expectedType ? `Expected: ${expectedType}` : undefined,
      };
    }

    return null;
  }

  /**
   * Validate action type and parameters
   */
  validateAction(actionType: string, parameters: Record<string, any>): ValidationError | null {
    if (!KNOWN_ACTIONS.includes(actionType)) {
      const suggestions = KNOWN_ACTIONS.filter((a) =>
        this.levenshteinDistance(a, actionType) < 3
      );
      let hint = '';
      if (suggestions.length > 0) {
        hint = `Did you mean: ${suggestions.slice(0, 3).join(', ')}?`;
      }
      return {
        message: `Unknown action type: '${actionType}'`,
        position: { line: 1, column: 1 },
        severity: 'error',
        hint,
      };
    }

    // Validate action-specific parameters
    const paramErrors = this.validateActionParameters(actionType, parameters);
    if (paramErrors.length > 0) {
      return {
        message: `Invalid parameters for action '${actionType}': ${paramErrors.join(', ')}`,
        position: { line: 1, column: 1 },
        severity: 'error',
      };
    }

    return null;
  }

  /**
   * Validate action-specific parameter requirements
   */
  private validateActionParameters(
    actionType: string,
    parameters: Record<string, any>
  ): string[] {
    const errors: string[] = [];

    switch (actionType) {
      case 'send_email':
        if (!parameters.to || !parameters.subject) {
          errors.push('Missing required parameters: to, subject');
        }
        break;
      case 'create_task':
        if (!parameters.title || !parameters.assignee) {
          errors.push('Missing required parameters: title, assignee');
        }
        break;
    }

    return errors;
  }

  /**
   * Suggest similar field names using edit distance
   */
  private suggestField(fieldName: string): string[] {
    if (!this.schema) return [];

    const suggestions: { field: string; distance: number }[] = [];
    for (const field of this.schema.fields) {
      const distance = this.levenshteinDistance(field.name, fieldName);
      if (distance <= 3 && distance < field.name.length / 2) {
        suggestions.push({ field: field.name, distance });
      }
    }

    return suggestions
      .sort((a, b) => a.distance - b.distance)
      .map((s) => s.field);
  }

  /**
   * Calculate Levenshtein distance between two strings
   */
  private levenshteinDistance(a: string, b: string): number {
    const matrix: number[][] = [];
    for (let i = 0; i <= a.length; i++) {
      matrix[i] = [i];
    }
    for (let j = 0; j <= b.length; j++) {
      matrix[0][j] = j;
    }

    for (let i = 1; i <= a.length; i++) {
      for (let j = 1; j <= b.length; j++) {
        if (a[i - 1] === b[j - 1]) {
          matrix[i][j] = matrix[i - 1][j - 1];
        } else {
          matrix[i][j] = Math.min(
            matrix[i - 1][j - 1] + 1, // substitution
            matrix[i][j - 1] + 1, // insertion
            matrix[i - 1][j] + 1 // deletion
          );
        }
      }
    }

    return matrix[a.length][b.length];
  }
}

/**
 * SyntaxValidator - Validates syntax in real-time as user types
 */
export class SyntaxValidator {
  /**
   * Create a CodeMirror lint provider for the inline validator
   */
  createLintProvider(): any {
    return {
      lint: (state: EditorState) => this.validateSyntax(state),
    };
  }

  /**
   * Validate syntax on document change
   */
  validateSyntax(state: EditorState): Diagnostic[] {
    const diagnostics: Diagnostic[] = [];
    const doc = state.doc;

    for (let lineNum = 1; lineNum <= doc.lines; lineNum++) {
      const line = doc.line(lineNum);
      const text = line.text.trim();
      const lineNumber = lineNum - 1; // Convert to 0-based index

      // Skip empty lines and comments
      if (!text || text.startsWith('#') || text.startsWith('//')) {
        continue;
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

      // Check for standalone operators (incomplete expressions)
      this.checkOperatorPlacement(text, line, diagnostics);

      // Check for common IF-THEN structure issues
      this.checkIfThenStructure(doc.toString(), lineNum, line, diagnostics);
    }

    return diagnostics;
  }

  /**
   * Check for invalid operator usage patterns
   */
  private checkOperatorPlacement(
    text: string,
    line: { from: number; to: number },
    diagnostics: Diagnostic[]
  ): void {
    const operators = ['>', '<', '==', '!=', '>=', '<='];

    for (const op of operators) {
      let index = text.indexOf(op);
      while (index !== -1) {
        const before = text.substring(0, index).trim();
        const after = text.substring(index + op.length).trim();

        // Check for standalone operator (no left operand)
        if (!before && !after) {
          diagnostics.push({
            from: line.from,
            to: line.to,
            message: `Standalone '${op}' operator - missing operands`,
            severity: 'error',
          });
        } else if (!before) {
          // Operator at start (e.g., "> 5" without left side)
          diagnostics.push({
            from: line.from + index,
            to: line.from + index + op.length,
            message: `Missing left operand for '${op}'`,
            severity: 'error',
          });
        } else if (!after) {
          // Operator at end (e.g., "x >" without right side)
          diagnostics.push({
            from: line.from + index,
            to: line.from + index + op.length,
            message: `Missing right operand for '${op}'`,
            severity: 'error',
          });
        }

        index = text.indexOf(op, index + 1);
      }
    }
  }

  /**
   * Check IF-THEN clause pairing across multiple lines
   */
  private checkIfThenStructure(
    fullText: string,
    lineNum: number,
    line: { from: number; to: number },
    diagnostics: Diagnostic[]
  ): void {
    const text = line.text.toUpperCase();

    if (text.trim().startsWith('IF')) {
      // Check if THEN appears later in the document
      const remainingText = fullText.substring(line.to).toUpperCase();
      const hasThen = remainingText.includes('\nTHEN') || remainingText.includes('\nTHEN ');

      if (!hasThen) {
        diagnostics.push({
          from: line.from,
          to: line.to,
          message: 'Missing THEN clause after IF',
          severity: 'warning',
        });
      }
    }
  }
}

/**
 * Main validateRule function - entry point for validation
 */
export function validateRule(
  ruleText: string,
  schema?: EntitySchema,
  mode: 'syntax' | 'semantic' | 'full' = 'syntax'
): ValidationResult {
  const result: ValidationResult = { errors: [], warnings: [], infos: [] };

  // Syntax validation (always runs)
  const syntaxValidator = new SyntaxValidator();
  const state = EditorState.create({ doc: ruleText }).state;
  const syntaxDiagnostics = syntaxValidator.validateSyntax(state);

  // Convert diagnostics to ValidationError format
  for (const diag of syntaxDiagnostics) {
    if (diag.severity === 'error') {
      result.errors.push({
        message: diag.message,
        position: getLineColumn(ruleText, diag.from),
        severity: 'error' as const,
      });
    } else {
      result.warnings.push({
        message: diag.message,
        position: getLineColumn(ruleText, diag.from),
        severity: 'warning' as const,
      });
    }
  }

  // Semantic validation (optional)
  if (mode === 'semantic' || mode === 'full') {
    if (schema) {
      const semanticValidator = new SemanticValidator(schema);
      const parsedRule = parseRuleStructure(ruleText);

      if (parsedRule.conditions.length > 0 || parsedRule.actions.length > 0) {
        const semanticResult = semanticValidator.validate(parsedRule);
        result.errors.push(...semanticResult.errors);
        result.warnings.push(...semanticResult.warnings);
        result.infos.push(...semanticResult.infos);
      }
    }
  }

  return result;
}

/**
 * Parse rule text into structured format for semantic validation
 */
function parseRuleStructure(ruleText: string): RuleStructure {
  const conditions: Condition[] = [];
  const actions: Action[] = [];

  const lines = ruleText.split('\n');
  let currentCondition: Partial<Condition> | null = null;

  for (const line of lines) {
    const trimmedLine = line.trim();

    // Skip empty lines and comments
    if (!trimmedLine || trimmedLine.startsWith('#') || trimmedLine.startsWith('//')) {
      continue;
    }

    const upperLine = trimmedLine.toUpperCase();

    // Parse condition lines (IF, AND, OR)
    if (upperLine.startsWith('IF ') || upperLine.startsWith('AND ') || upperLine.startsWith('OR ')) {
      if (currentCondition) {
        conditions.push(currentCondition as Condition);
        currentCondition = null;
      }

      const operatorMatch = trimmedLine.match(/(>=|<=|==|!=|>|<)\s*(.*)/);
      if (operatorMatch) {
        const parts = trimmedLine.split(/\s+(?:==|!=|>|<|>=|<=)\s+/);
        if (parts.length >= 2) {
          currentCondition = {
            field: parts[0],
            operator: operatorMatch[1],
            value: parts.slice(1).join(' '),
          };
        }
      }
    }

    // Parse action lines (THEN, APPLY, SEND, etc.)
    if (upperLine.startsWith('THEN ') || upperLine.includes(' THEN ')) {
      const actionMatch = trimmedLine.match(/THEN\s+(.+?)(?:;|$)/);
      if (actionMatch) {
        actions.push({
          type: actionMatch[1].trim(),
          parameters: {},
        });
      }
    }
  }

  // Don't forget the last condition
  if (currentCondition) {
    conditions.push(currentCondition as Condition);
  }

  return { conditions, actions };
}

/**
 * Convert character position to line/column for error display
 */
function getLineColumn(text: string, position: number): { line: number; column: number } {
  const lines = text.substring(0, position).split('\n');
  return {
    line: lines.length,
    column: lines[lines.length - 1].length + 1,
  };
}

export default validateRule;
