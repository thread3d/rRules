// Hybrid accordion component with expandable/collapsible sections and collapse-after-review

import React, {
  createContext,
  useContext,
  useState,
  useCallback,
  ReactNode,
} from 'react';

/**
 * Context for accordion state management
 */
interface AccordionContextType {
  expandedSections: Set<string>;
  reviewedSections: Set<string>;
  toggleSection: (id: string) => void;
  markAsReviewed: (id: string) => void;
  collapseAll: () => void;
  expandAll: () => void;
}

const AccordionContext = createContext<AccordionContextType | undefined>(undefined);

/**
 * AccordionProvider component
 */
export function AccordionProvider({ children }: { children: ReactNode }) {
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set());
  const [reviewedSections, setReviewedSections] = useState<Set<string>>(new Set());

  const toggleSection = useCallback((id: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const markAsReviewed = useCallback((id: string) => {
    setReviewedSections((prev) => new Set(prev).add(id));
  }, []);

  const collapseAll = useCallback(() => {
    setExpandedSections(new Set());
  }, []);

  const expandAll = useCallback(() => {
    // In hybrid mode, all start expanded - this is mainly for user-triggered expansion
  }, []);

  return (
    <AccordionContext.Provider
      value={{
        expandedSections,
        reviewedSections,
        toggleSection,
        markAsReviewed,
        collapseAll,
        expandAll,
      }}
    >
      {children}
    </AccordionContext.Provider>
  );
}

/**
 * useAccordionState hook
 */
export function useAccordionState() {
  const context = useContext(AccordionContext);
  if (!context) {
    throw new Error('useAccordionState must be used within an AccordionProvider');
  }
  return context;
}

/**
 * Props for Accordion component
 */
interface AccordionProps {
  children: ReactNode;
  className?: string;
}

export function Accordion({ children, className = '' }: AccordionProps) {
  return (
    <AccordionProvider>
      <div className={`accordion ${className}`}>{children}</div>
    </AccordionProvider>
  );
}

/**
 * Props for AccordionSection component
 */
interface AccordionSectionProps {
  id: string;
  title: ReactNode;
  description?: string;
  children: ReactNode;
  isCompleted?: boolean;
  requiredFieldsFilled?: boolean;
  className?: string;
}

export function AccordionSection({
  id,
  title,
  description,
  children,
  isCompleted = false,
  requiredFieldsFilled = true,
  className = '',
}: AccordionSectionProps) {
  const context = useAccordionState();
  const isExpanded = context.expandedSections.has(id);
  const isReviewed = context.reviewedSections.has(id);

  // Auto-collapse after review
  React.useEffect(() => {
    if (isReviewed && requiredFieldsFilled) {
      // Optional: auto-collapse logic could go here
    }
  }, [isReviewed, requiredFieldsFilled]);

  const handleToggle = useCallback(() => {
    context.toggleSection(id);
  }, [context, id]);

  const handleMarkAsReviewed = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      context.markAsReviewed(id);
    },
    [context, id]
  );

  // Determine visual state
  let visualState = 'expanded';
  if (!isExpanded && isReviewed) {
    visualState = 'reviewed';
  } else if (!isExpanded) {
    visualState = 'collapsed';
  }

  return (
    <div className={`accordion-section ${visualState} ${className}`}>
      {/* Header */}
      <div
        className="accordion-header"
        onClick={handleToggle}
        role="button"
        aria-expanded={isExpanded}
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            handleToggle();
          }
        }}
      >
        <div className="accordion-title-area">
          <span className={`chevron ${isExpanded ? 'expanded' : ''}`}>
            {isExpanded ? (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path d="M6 9l6 6 6-6" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            ) : (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path d="M9 18l6-6-6-6" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            )}
          </span>
          <div className="accordion-header-content">
            <h3 className="accordion-title">{title}</h3>
            {description && <p className="accordion-description">{description}</p>}
          </div>
        </div>

        <div className="accordion-actions">
          {/* Review badge if completed */}
          {isReviewed ? (
            <span className="review-badge" title="Section reviewed">
              ✓
            </span>
          ) : (
            <button
              type="button"
              onClick={handleMarkAsReviewed}
              className="mark-reviewed-btn"
            >
              I've reviewed
            </button>
          )}
        </div>
      </div>

      {/* Content */}
      {isExpanded && (
        <div className="accordion-content">
          {children}
        </div>
      )}

      {/* Compact card view when collapsed and reviewed */}
      {!isExpanded && isReviewed && (
        <div className="accordion-reviewed-card" onClick={handleToggle} role="button">
          <span className="checkmark">✓</span>
          <span className="card-title">{title}</span>
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              context.toggleSection(id);
            }}
            className="expand-btn"
          >
            Expand
          </button>
        </div>
      )}
    </div>
  );
}

/**
 * Nested accordion support for complex rules
 */
export function NestedAccordion({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <div className={`nested-accordion ${className}`}>{children}</div>;
}

export default Accordion;
