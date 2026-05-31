// Version tracking and rollback interface for templates

import React, { useState, useEffect, useCallback } from 'react';
import { Template } from './template-system';

/**
 * Template version structure
 */
export interface TemplateVersion {
  version: number;
  templateId: string;
  author: string;
  timestamp: string;
  changes: ChangeSummary[];
  diff: TemplateDiff;
}

/**
 * Change summary for a version
 */
export interface ChangeSummary {
  field: keyof Template | 'parameters';
  old: any;
  new: any;
}

/**
 * Diff representation
 */
export interface TemplateDiff {
  additions: number;
  deletions: number;
  modified: number;
}

/**
 * Props for VersionHistory component
 */
interface VersionHistoryProps {
  templateId: string;
  versions: TemplateVersion[];
  currentTemplate: Template;
  onRollback?: (version: TemplateVersion) => void;
}

/**
 * VersionHistory - Displays version timeline and rollback interface
 */
export function VersionHistory({
  templateId,
  versions,
  currentTemplate,
  onRollback,
}: VersionHistoryProps) {
  const [selectedVersion, setSelectedVersion] = useState<TemplateVersion | null>(null);
  const [showDiff, setShowDiff] = useState(false);

  // Sort versions by version number descending
  const sortedVersions = [...versions].sort((a, b) => b.version - a.version);

  const handleSelectVersion = useCallback(
    (version: TemplateVersion) => {
      setSelectedVersion(version);
      setShowDiff(true);
    },
    []
  );

  const handleRollback = useCallback(() => {
    if (selectedVersion && onRollback) {
      onRollback(selectedVersion);
    }
  }, [selectedVersion, onRollback]);

  // Calculate diff stats
  const calculateDiffStats = useCallback((version: TemplateVersion): TemplateDiff => {
    let additions = 0;
    let deletions = 0;
    let modified = 0;

    for (const change of version.changes) {
      if (
        change.old === undefined &&
        change.new !== undefined &&
        change.new !== null
      ) {
        additions++;
      } else if (
        change.old !== undefined &&
        change.old !== null &&
        change.new === undefined
      ) {
        deletions++;
      } else {
        modified++;
      }
    }

    return { additions, deletions, modified };
  }, []);

  // Render version timeline
  const renderVersionTimeline = () => (
    <div className="version-timeline">
      <div className="timeline-header">Version History</div>

      {sortedVersions.map((version) => {
        const diffStats = calculateDiffStats(version);
        const isSelected = selectedVersion?.version === version.version;

        return (
          <div
            key={version.version}
            className={`version-item ${isSelected ? 'selected' : ''}`}
            onClick={() => handleSelectVersion(version)}
          >
            <div className="version-header">
              <span className="version-number">v{version.version}</span>
              <span className="author">{version.author}</span>
              <span className="timestamp">
                {new Date(version.timestamp).toLocaleString()}
              </span>
            </div>

            <div className="version-summary">
              <span className="diff-stats">
                +{diffStats.additions} -{diffStats.deletions} ~{diffStats.modified}
              </span>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setShowDiff(!showDiff);
                }}
                className="toggle-diff-btn"
              >
                {showDiff ? 'Hide Diff' : 'Show Diff'}
              </button>
            </div>

            {showDiff && (
              <div className="diff-preview">
                <ul className="changes-list">
                  {version.changes.slice(0, 5).map((change, i) => (
                    <li key={i} className={`change-item ${getChangeType(change)}`}>
                      <span className="field">{change.field}</span>
                      <div className="diff-lines">
                        {change.old !== undefined && (
                          <span className="line old">- {String(change.old)}</span>
                        )}
                        {change.new !== undefined && change.new !== null && (
                          <span className="line new">+ {String(change.new)}</span>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );

  // Render rollback UI
  const renderRollbackUI = () => {
    if (!selectedVersion) return null;

    const diffStats = calculateDiffStats(selectedVersion);

    return (
      <div className="rollback-panel">
        <h4>Rollback to v{selectedVersion.version}</h4>
        <div className="preview-diff">
          <p>Additions: +{diffStats.additions}, Deletions: -{diffStats.deletions}</p>
        </div>

        <button
          type="button"
          onClick={handleRollback}
          className="rollback-btn"
        >
          Rollback to this version
        </button>
      </div>
    );
  };

  return (
    <div className="version-history">
      {renderVersionTimeline()}
      {renderRollbackUI()}
    </div>
  );
}

/**
 * Helper to determine change type for styling
 */
function getChangeType(change: ChangeSummary): 'added' | 'removed' | 'modified' {
  if (change.old === undefined || change.old === null) {
    return 'added';
  }
  if (change.new === undefined || change.new === null) {
    return 'removed';
  }
  return 'modified';
}

/**
 * Props for compareVersions component
 */
interface CompareVersionsProps {
  version1: TemplateVersion;
  version2: TemplateVersion;
}

export function CompareVersions({ version1, version2 }: CompareVersionsProps) {
  const [showDiff, setShowDiff] = useState(true);

  // Calculate which fields differ between versions
  const getDifferences = useCallback((): ChangeSummary[] => {
    const changes: ChangeSummary[] = [];
    const allFields = new Set([
      ...version1.changes.map((c) => c.field),
      ...version2.changes.map((c) => c.field),
    ]);

    for (const field of allFields) {
      const v1Change = version1.changes.find((c) => c.field === field);
      const v2Change = version2.changes.find((c) => c.field === field);

      if (
        v1Change &&
        v2Change &&
        JSON.stringify(v1Change.new) !== JSON.stringify(v2Change.new)
      ) {
        changes.push({
          field,
          old: v1Change.new,
          new: v2Change.new,
        });
      }
    }

    return changes;
  }, [version1, version2]);

  const differences = getDifferences();

  return (
    <div className="compare-versions">
      <div className="compare-header">
        <span>v{version1.version}</span>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
          <path d="M5 12h14M12 5l7 7-7 7" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <span>v{version2.version}</span>
      </div>

      <div className="compare-content">
        {showDiff ? (
          <ul className="diff-list">
            {differences.map((change, i) => (
              <li key={i} className={`diff-row ${getChangeType(change)}`}>
                <span className="field-name">{change.field}</span>
                <div className="diff-values">
                  <span className="old-value">Old: {String(change.old)}</span>
                  <span className="new-value">New: {String(change.new)}</span>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <div className="version-summary">
            <p>v{version1.version}: {version1.changes.length} changes</p>
            <p>v{version2.version}: {version2.changes.length} changes</p>
          </div>
        )}
      </div>

      <button type="button" onClick={() => setShowDiff(!showDiff)}>
        Toggle Diff View
      </button>
    </div>
  );
}

/**
 * Compare any two templates by version
 */
export function compareVersions(
  version1: TemplateVersion,
  version2: TemplateVersion
): boolean {
  return (
    JSON.stringify(version1.changes) === JSON.stringify(version2.changes) &&
    version1.timestamp === version2.timestamp
  );
}

/**
 * Rollback to a specific version
 */
export async function rollbackToVersion(
  templateId: string,
  version: TemplateVersion,
  onUpdate: (updatedTemplate: Template) => void
): Promise<Template | null> {
  // In production, this would call Phase 3 API
  // For now, simulate rollback by creating a new version with rolled-back content

  const newTemplate: Template = {
    id: templateId,
    name: version.changes.find((c) => c.field === 'name')?.new || 'Unknown',
    description:
      version.changes.find((c) => c.field === 'description')?.new || '',
    parameters: [],
    baseStructure: { type: 'rule', children: [] },
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    owner: 'currentUser',
    isShared: false,
  };

  onUpdate(newTemplate);
  return newTemplate;
}

export default VersionHistory;
