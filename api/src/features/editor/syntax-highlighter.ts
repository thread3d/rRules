// Syntax highlighter for the rRules DSL using CodeMirror 6

import { LanguageSupport, LRLanguage } from '@codemirror/language';
import { styleTags, tags as t } from '@lezer/highlight';

/**
 * Token types for the rRules DSL
 */
export const tokenTypes = {
  keyword: 'keyword',
  function: 'function',
  variable: 'variable',
  literal: 'literal',
  operator: 'operator',
  comment: 'comment',
};

/**
 * DSL Keywords
 */
export const DSL_KEYWORDS = [
  'IF', 'THEN', 'ELSE', 'WHEN', 'AND', 'OR', 'NOT',
  'TRUE', 'FALSE', 'NULL',
];

/**
 * Operators
 */
export const OPERATORS = ['>', '<', '==', '!=', '>=', '<=', '=', '+', '-', '*', '/'];

/**
 * Create the rRules language definition for CodeMirror 6
 */
export const ruleLanguage = LRLanguage.define({
  parser: {
    grammar: {
      rules: [
        // Keywords
        { token: 'keyword', words: DSL_KEYWORDS },

        // Operators
        { token: 'operator', match: /^(>=|<=|==|!=|>|<|=|\+|-|\*|\/)$/ },

        // Variables (placeholders)
        { token: 'variable', match: /^\{[a-zA-Z_][a-zA-Z0-9_]*\}$/ },

        // Literals
        { token: 'literal', match: /^"[^"]*"$|^\d+(\.\d+)?$/ },

        // Comments
        { token: 'comment', match: /^(#|\/\/).*$/ },

        // Punctuation
        { token: 'bracket', match: /^[{}[\](),:;]$/ },
      ],
    },
  },
});

/**
 * Syntax highlighting extension for CodeMirror 6
 */
export const ruleHighlighting = ruleLanguage.data.of({
  highlight: styleTags({
    [tokenTypes.keyword]: t.keyword,
    [tokenTypes.function]: t.function,
    [tokenTypes.variable]: t.variableName,
    [tokenTypes.literal]: t.string,
    [tokenTypes.operator]: t.operator,
    [tokenTypes.comment]: t.lineComment,
  }),
});

/**
 * Create a LanguageSupport instance for the rRules DSL
 */
export function createRuleLanguageSupport(): LanguageSupport {
  return new LanguageSupport(ruleLanguage, [ruleHighlighting]);
}
