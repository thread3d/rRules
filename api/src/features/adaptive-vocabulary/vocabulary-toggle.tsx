// Technical-to-business vocabulary switching system with toggle control

import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
} from 'react';

/**
 * Vocabulary mappings between technical and business terms
 */
export const VOCABULARY = {
  technical: {
    field: 'field',
    value: 'value',
    operator: 'operator',
    action: 'action',
    condition: 'condition',
    entity: 'entity',
    threshold: 'threshold',
    comparison: 'comparison type',
    ruleName: 'rule name',
    description: 'description',
  },
  business: {
    field: 'Customer Property',
    value: 'Threshold Value',
    operator: 'Comparison Type',
    action: 'Action',
    condition: 'Condition Clause',
    entity: 'Business Entity',
    threshold: 'Value Threshold',
    comparison: 'How to Compare',
    ruleName: 'Rule Title',
    description: 'Rule Purpose',
  },
};

/**
 * Context for vocabulary selection
 */
interface VocabularyContextType {
  mode: 'technical' | 'business';
  toggleMode: () => void;
  setMode: (mode: 'technical' | 'business') => void;
  translate: (term: keyof typeof VOCABULARY.technical) => string;
}

const VocabularyContext = createContext<VocabularyContextType | undefined>(undefined);

/**
 * VocabularyToggleProvider component
 */
export function VocabularyToggleProvider({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<'technical' | 'business'>('technical');

  // Load preference from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem('vocabularyMode');
    if (stored === 'business') {
      setMode('business');
    }
  }, []);

  // Save preference to localStorage when mode changes
  useEffect(() => {
    localStorage.setItem('vocabularyMode', mode);
  }, [mode]);

  const toggleMode = useCallback(() => {
    setMode((prev) => (prev === 'technical' ? 'business' : 'technical'));
  }, []);

  const translate = useCallback(
    (term: keyof typeof VOCABULARY.technical): string => {
      return VOCABULARY[mode][term] || VOCABULARY.technical[term];
    },
    [mode]
  );

  return (
    <VocabularyContext.Provider
      value={{ mode, toggleMode, setMode, translate }}
    >
      {children}
    </VocabularyContext.Provider>
  );
}

/**
 * useVocabularyContext hook
 */
export function useVocabularyContext() {
  const context = useContext(VocabularyContext);
  if (!context) {
    throw new Error('useVocabularyContext must be used within a VocabularyToggleProvider');
  }
  return context;
}

/**
 * VocabularyToggle component - Toggle button for vocabulary mode
 */
export function VocabularyToggle({ className = '' }: { className?: string }) {
  const { mode, toggleMode } = useVocabularyContext();

  return (
    <button
      type="button"
      onClick={toggleMode}
      className={`vocabulary-toggle ${mode} ${className}`}
      title={`Switch to ${mode === 'technical' ? 'business' : 'technical'} terms`}
    >
      <span className="current-mode">{mode === 'technical' ? 'Tech' : 'Business'}</span>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
        <path d="M4 12h16M12 4l8 8-8 8" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

/**
 * AdaptiveLabel component - Displays translated term based on vocabulary context
 */
export function AdaptiveLabel({
  term,
  children,
}: {
  term: keyof typeof VOCABULARY.technical;
  children?: ReactNode;
}) {
  const { translate } = useVocabularyContext();
  const text = children || translate(term);

  return <span className="adaptive-label">{text}</span>;
}

/**
 * FormLabel wrapper with adaptive vocabulary
 */
export function FormLabel({
  htmlFor,
  term,
  required = false,
}: {
  htmlFor: string;
  term: keyof typeof VOCABULARY.technical;
  required?: boolean;
}) {
  const { translate } = useVocabularyContext();
  return (
    <label htmlFor={htmlFor} className="form-label">
      {translate(term)}
      {required && <span className="required-asterisk">*</span>}
    </label>
  );
}

/**
 * HelpText wrapper with adaptive vocabulary
 */
export function HelpText({ term }: { term: keyof typeof VOCABULARY.technical }) {
  const { translate } = useVocabularyContext();
  return <p className="help-text">{translate(term)}</p>;
}

// Re-export for convenience
export { VocabularyToggle, AdaptiveLabel, FormLabel, HelpText };
