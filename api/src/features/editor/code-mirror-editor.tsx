// CodeMirror 6 editor component for the rRules DSL

import { EditorState, EditorView, keymap, history } from '@codemirror/view';
import { EditorState as State, Extension, Command } from '@codemirror/state';
import { defaultKeymap, indentWithTab } from '@codemirror/commands';
import { history as historyExt } from '@codemirror/history';
import { indentUnit } from '@codemirror/language';
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
import { autocompletion } from '@codemirror/autocomplete';
import { lintView } from '@codemirror/lint';

import { ruleLanguage, createRuleLanguageSupport } from './syntax-highlighter';
import { createRuleLinter, validateRuleDocument, SyntaxError } from '../validation/realtime-validator';

/**
 * Props for the CodeMirrorEditor component
 */
export interface CodeMirrorEditorProps {
  initialValue?: string;
  onChange?: (value: string) => void;
  onValidate?: (diagnostics: SyntaxError[]) => void;
  className?: string;
}

/**
 * Hook to manage the CodeMirror editor state
 */
export function useRuleEditor(initialValue = '') {
  const [editorState, setEditorState] = useState<State | null>(null);

  useEffect(() => {
    const startState = EditorState.create({
      doc: initialValue,
      extensions: [
        ruleLanguage,
        createRuleLanguageSupport(),
        historyExt(),
        lintView(createRuleLinter()),
        keymap.of([...defaultKeymap, ...searchKeymap]),
        indentUnit.of('  '),
        autocompletion(),
        highlightSelectionMatches(),
        EditorState.allowMultipleSelections,
      ],
    });
    setEditorState(startState);
  }, [initialValue]);

  return { editorState, setEditorState };
}

/**
 * CodeMirrorEditor React component
 */
export function CodeMirrorEditor({
  initialValue = '',
  onChange,
  onValidate,
  className = '',
}: CodeMirrorEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const { editorState, setEditorState } = useRuleEditor(initialValue);

  useEffect(() => {
    if (containerRef.current && editorState) {
      viewRef.current = new EditorView({
        state: editorState,
        parent: containerRef.current,
      });

      // Set up change handler
      viewRef.current.dispatch({
        changes: { from: 0, to: viewRef.current.state.doc.length, insert: initialValue },
      });

      return () => {
        if (viewRef.current) {
          viewRef.current.destroy();
          viewRef.current = null;
        }
      };
    }
  }, [editorState]);

  useEffect(() => {
    if (viewRef.current && onValidate) {
      const diagnostics = validateRuleDocument(viewRef.current.state.doc.toString());
      onValidate(diagnostics);
    }
  }, [onValidate]);

  // Exposed methods
  const getValue = () => viewRef.current?.state.doc.toString() || '';
  const setValue = (text: string) => {
    if (viewRef.current) {
      viewRef.current.dispatch({
        changes: { from: 0, to: viewRef.current.state.doc.length, insert: text },
      });
    }
  };
  const validateSyntax = () => {
    if (viewRef.current) {
      return validateRuleDocument(viewRef.current.state.doc.toString());
    }
    return [];
  };

  return (
    <div ref={containerRef} className={`cm-editor ${className}`}>
      {/* Editor will be rendered here */}
    </div>
  );
}

// Re-export for convenience
export { createRuleLinter, validateRuleDocument, SyntaxError };
