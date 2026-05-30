// Dual-pane view layout component for editor and guided builder

import React, { useState, useCallback } from 'react';

/**
 * Props for the DualPaneView component
 */
export interface DualPaneViewProps {
  leftPaneContent: React.ReactNode;
  rightPaneContent: React.ReactNode;
  showLeftPane?: boolean;
  showRightPane?: boolean;
  sharedToolbar?: React.ReactNode;
  onToggleLeft?: (show: boolean) => void;
  onToggleRight?: (show: boolean) => void;
  className?: string;
}

/**
 * DualPaneView - Side-by-side layout component for editor and guided builder
 */
export function DualPaneView({
  leftPaneContent,
  rightPaneContent,
  showLeftPane = true,
  showRightPane = true,
  sharedToolbar,
  onToggleLeft,
  onToggleRight,
  className = '',
}: DualPaneViewProps) {
  const [leftVisible, setLeftVisible] = useState(showLeftPane);
  const [rightVisible, setRightVisible] = useState(showRightPane);

  const handleToggleLeft = useCallback(() => {
    const newState = !leftVisible;
    setLeftVisible(newState);
    onToggleLeft?.(newState);
  }, [leftVisible, onToggleLeft]);

  const handleToggleRight = useCallback(() => {
    const newState = !rightVisible;
    setRightVisible(newState);
    onToggleRight?.(newState);
  }, [rightVisible, onToggleRight]);

  // Determine column widths based on visibility
  const leftWidth = leftVisible && rightVisible ? '1fr' : leftVisible ? '100%' : '0';
  const rightWidth = leftVisible && rightVisible ? '1fr' : rightVisible ? '100%' : '0';

  return (
    <div className={`dual-pane-view ${className}`}>
      {/* Toolbar */}
      {sharedToolbar && (
        <div className="dual-pane-toolbar">
          {sharedToolbar}
        </div>
      )}

      {/* Main content area with CSS Grid layout */}
      <div
        className="dual-pane-container"
        style={{
          display: 'grid',
          gridTemplateColumns: `${leftWidth} ${rightWidth}`,
          gap: '1rem',
          minHeight: '0', // Important for flex children to shrink
        }}
      >
        {/* Left Pane - Code Editor */}
        <div className={`dual-pane-left ${leftVisible ? '' : 'hidden'}`}>
          {leftPaneContent}
        </div>

        {/* Right Pane - Guided Builder */}
        <div className={`dual-pane-right ${rightVisible ? '' : 'hidden'}`}>
          {rightPaneContent}
        </div>
      </div>

      {/* Toggle buttons for accessibility */}
      {(showLeftPane || showRightPane) && (
        <div className="dual-pane-toggles">
          {showLeftPane && (
            <button
              type="button"
              onClick={handleToggleLeft}
              aria-pressed={leftVisible}
              className={`pane-toggle pane-left ${leftVisible ? 'active' : ''}`}
            >
              {leftVisible ? 'Hide Editor' : 'Show Editor'}
            </button>
          )}
          {showRightPane && (
            <button
              type="button"
              onClick={handleToggleRight}
              aria-pressed={rightVisible}
              className={`pane-toggle pane-right ${rightVisible ? 'active' : ''}`}
            >
              {rightVisible ? 'Hide Builder' : 'Show Builder'}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * EditorPane - Wrapper for the code editor
 */
export interface EditorPaneProps {
  content: React.ReactNode;
  title?: string;
  className?: string;
}

export function EditorPane({ content, title = 'Rule Editor', className = '' }: EditorPaneProps) {
  return (
    <div className={`editor-pane ${className}`}>
      {title && <h2>{title}</h2>}
      <div className="editor-content">{content}</div>
    </div>
  );
}

/**
 * GuidedBuilderPane - Wrapper for the guided rule builder
 */
export interface GuidedBuilderPaneProps {
  content: React.ReactNode;
  title?: string;
  className?: string;
}

export function GuidedBuilderPane({ content, title = 'Guided Builder', className = '' }: GuidedBuilderPaneProps) {
  return (
    <div className={`guided-builder-pane ${className}`}>
      {title && <h2>{title}</h2>}
      <div className="builder-content">{content}</div>
    </div>
  );
}

export default DualPaneView;
