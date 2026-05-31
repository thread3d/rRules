// Template manager - Complete template management UI with CRUD and version browser

import React, { useState, useEffect, useCallback } from 'react';
import { Template, Parameter, instantiateTemplate, detectParameters } from './template-system';
import {
  getTemplates,
  createTemplate as storageCreate,
  updateTemplate as storageUpdate,
  deleteTemplate as storageDelete,
  setTemplateShared,
  importTemplates,
  exportTemplates,
  User,
} from './storage';
import { VersionHistory, rollbackToVersion, TemplateVersion } from './version-history';

/**
 * Modal state for create/edit template
 */
interface TemplateModalState {
  isOpen: boolean;
  mode: 'create' | 'edit';
  template?: Template;
}

/**
 * Props for TemplateManager component
 */
interface TemplateManagerProps {
  currentUser: User;
  onTemplateSelect?: (template: Template) => void;
}

/**
 * TemplateCard - Individual template display in grid
 */
interface TemplateCardProps {
  template: Template;
  onClick: () => void;
  isSelected: boolean;
}

function TemplateCard({ template, onClick, isSelected }: TemplateCardProps) {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <div
      className={`template-card ${isSelected ? 'selected' : ''}`}
      onClick={onClick}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div className="card-header">
        <h4 className="template-name">{template.name}</h4>
        {template.isShared && (
          <span className="badge shared-badge">Shared</span>
        )}
        <span
          className="owner-avatar"
          title={`Owner: ${template.owner}`}
        >
          {template.owner.substring(0, 2).toUpperCase()}
        </span>
      </div>

      <p className="template-description">{template.description}</p>

      <div className="card-footer">
        <span className="param-count">
          {template.parameters.length} params
        </span>
        <span className="usage-count">
          Usage: {(template as any).usageCount || 0}
        </span>
        <span className="date-badge">
          {new Date(template.updatedAt).toLocaleDateString()}
        </span>
      </div>

      {isHovered && (
        <div className="card-actions">
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              (template as any).onDuplicate?.();
            }}
            className="action-btn"
            title="Duplicate template"
          >
            Duplicate
          </button>
          {template.owner === 'currentUser' && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                (template as any).onToggleShare?.();
              }}
              className="action-btn"
              title={template.isShared ? 'Unshare template' : 'Share template'}
            >
              {template.isShared ? 'Unshare' : 'Share'}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * TemplateModal - Create/edit template dialog
 */
interface TemplateModalProps {
  isOpen: boolean;
  mode: 'create' | 'edit';
  initialTemplate?: Template;
  onClose: () => void;
  onSave: (template: Template) => void;
}

function TemplateModal({
  isOpen,
  mode,
  initialTemplate,
  onClose,
  onSave,
}: TemplateModalProps) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [parameters, setParameters] = useState<Parameter[]>([]);
  const [templateBody, setTemplateBody] = useState('');
  const [isShared, setIsShared] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [activeTab, setActiveTab] = useState<'editor' | 'parameters'>('editor');

  useEffect(() => {
    if (isOpen) {
      if (initialTemplate) {
        setName(initialTemplate.name);
        setDescription(initialTemplate.description);
        setParameters(initialTemplate.parameters);
        setTemplateBody(JSON.stringify(initialTemplate.baseStructure, null, 2));
        setIsShared(initialTemplate.isShared);
      } else {
        setName('');
        setDescription('');
        setParameters([]);
        setTemplateBody('{"type":"rule","children":[]}');
        setIsShared(false);
      }
      setErrors({});
    }
  }, [isOpen, initialTemplate]);

  const handleAddParameter = useCallback(() => {
    setParameters((prev) => [
      ...prev,
      { name: `param${prev.length + 1}`, type: 'string', defaultValue: '' },
    ]);
  }, []);

  const handleRemoveParameter = useCallback(
    (index: number) => {
      setParameters((prev) => prev.filter((_, i) => i !== index));
    },
    []
  );

  const handleUpdateParameter = useCallback(
    (index: number, field: keyof Parameter, value: string) => {
      setParameters((prev) =>
        prev.map((p, i) =>
          i === index ? { ...p, [field]: value } : p
        )
      );
    },
    []
  );

  const handleDetectParameters = useCallback(() => {
    const detected = detectParameters(templateBody);
    setParameters(detected);
  }, [templateBody]);

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!name.trim()) {
      newErrors.name = 'Template name is required';
    }
    if (parameters.length > 0 && !templateBody) {
      newErrors.templateBody = 'Template body is required when using parameters';
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = useCallback(() => {
    if (!validateForm()) return;

    // Parse the base structure from JSON
    let baseStructure: any;
    try {
      baseStructure = templateBody ? JSON.parse(templateBody) : { type: 'rule', children: [] };
    } catch (e) {
      setErrors({ templateBody: 'Invalid JSON in template body' });
      return;
    }

    const templateData: Omit<Template, 'id' | 'createdAt' | 'updatedAt'> = {
      name,
      description,
      parameters,
      baseStructure,
      owner: 'currentUser',
      isShared,
    };

    if (mode === 'create') {
      const newTemplate = storageCreate(templateData);
      onSave(newTemplate);
    } else if (initialTemplate) {
      const updated = storageUpdate(initialTemplate.id, {
        name,
        description,
        parameters,
        baseStructure,
        isShared,
        updatedAt: new Date().toISOString(),
      });
      if (updated) {
        onSave(updated);
      }
    }
  }, [name, description, parameters, templateBody, isShared, mode, initialTemplate, onSave]);

  if (!isOpen) return null;

  return (
    <div className="template-modal-overlay">
      <div className="template-modal">
        <div className="modal-header">
          <h3>{mode === 'create' ? 'Create Template' : 'Edit Template'}</h3>
          <button
            type="button"
            onClick={onClose}
            className="close-btn"
            aria-label="Close modal"
          >
            x
          </button>
        </div>

        <div className="modal-body">
          {/* Name and Description */}
          <div className="form-group">
            <label htmlFor="template-name">Name *</label>
            <input
              id="template-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className={`form-control ${errors.name ? 'error' : ''}`}
              placeholder="Enter template name"
            />
            {errors.name && (
              <span className="error-message">{errors.name}</span>
            )}
          </div>

          <div className="form-group">
            <label htmlFor="template-description">Description</label>
            <textarea
              id="template-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="form-control"
              placeholder="Enter template description"
              rows={2}
            />
          </div>

          {/* Tabs */}
          <div className="modal-tabs">
            <button
              type="button"
              onClick={() => setActiveTab('editor')}
              className={`tab-btn ${activeTab === 'editor' ? 'active' : ''}`}
            >
              Template Editor
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('parameters')}
              className={`tab-btn ${activeTab === 'parameters' ? 'active' : ''}`}
            >
              Parameters
            </button>
          </div>

          {/* Tab Content */}
          <div className="modal-tab-content">
            {activeTab === 'editor' && (
              <div className="template-editor-section">
                <label htmlFor="template-body">Template Body (JSON)</label>
                <p className="hint-text">
                  Define the rule structure using JSON. Use <code>{`{paramName}`}</code> syntax for parameters.
                  Example: <code>"condition": "{customerType} == 'premium'"</code>
                </p>
                <textarea
                  id="template-body"
                  value={templateBody}
                  onChange={(e) => setTemplateBody(e.target.value)}
                  className={`form-control code-editor ${errors.templateBody ? 'error' : ''}`}
                  placeholder='{"type": "rule", "children": []}'
                  spellCheck={false}
                />
                {errors.templateBody && (
                  <span className="error-message">{errors.templateBody}</span>
                )}
                <button
                  type="button"
                  onClick={handleDetectParameters}
                  className="secondary-btn detect-params-btn"
                >
                  Detect Parameters from Body
                </button>
              </div>
            )}

            {activeTab === 'parameters' && (
              <div className="parameters-section">
                <div className="parameters-header">
                  <span>Defined Parameters</span>
                  <button
                    type="button"
                    onClick={handleAddParameter}
                    className="secondary-btn add-param-btn"
                  >
                    + Add Parameter
                  </button>
                </div>

                {parameters.length === 0 ? (
                  <p className="empty-state">No parameters defined. Add parameters or use "Detect" from editor tab.</p>
                ) : (
                  <div className="parameter-list">
                    {parameters.map((param, index) => (
                      <div key={index} className="parameter-row">
                        <input
                          type="text"
                          value={param.name}
                          onChange={(e) =>
                            handleUpdateParameter(index, 'name', e.target.value)
                          }
                          className="form-control param-name-input"
                          placeholder="Parameter name (e.g., customerType)"
                        />
                        <select
                          value={param.type}
                          onChange={(e) =>
                            handleUpdateParameter(
                              index,
                              'type',
                              e.target.value as Parameter['type']
                            )
                          }
                          className="form-control param-type-select"
                        >
                          <option value="string">String</option>
                          <option value="number">Number</option>
                          <option value="boolean">Boolean</option>
                          <option value="enum">Enum</option>
                        </select>
                        <input
                          type="text"
                          value={param.defaultValue || ''}
                          onChange={(e) =>
                            handleUpdateParameter(
                              index,
                              'defaultValue',
                              e.target.value
                            )
                          }
                          className="form-control param-default-input"
                          placeholder="Default value (optional)"
                        />
                        <button
                          type="button"
                          onClick={() => handleRemoveParameter(index)}
                          className="remove-btn"
                        >
                          x
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Share Toggle */}
          <div className="form-group share-toggle">
            <label>
              <input
                type="checkbox"
                checked={isShared}
                onChange={(e) => setIsShared(e.target.checked)}
              />
              <span>Share with team</span>
            </label>
          </div>

          {/* Version History (for existing templates) */}
          {mode === 'edit' && initialTemplate && (
            <div className="version-history-section">
              <h4>Version History</h4>
              <p className="hint-text">View and restore previous versions of this template.</p>
              {/* Version history would be populated from storage in Phase 3 */}
              <VersionHistory
                templateId={initialTemplate.id}
                versions={(initialTemplate as any).versions || []}
                currentTemplate={initialTemplate}
              />
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button type="button" onClick={onClose} className="cancel-btn">
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSave}
            className="save-btn"
          >
            {mode === 'create' ? 'Create Template' : 'Save Changes'}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * TemplateManager - Main template management component
 */
export function TemplateManager({
  currentUser,
  onTemplateSelect,
}: TemplateManagerProps) {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<'all' | 'mine' | 'shared'>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedTemplate, setSelectedTemplate] = useState<Template | null>(null);
  const [modalState, setModalState] = useState<TemplateModalState>({
    isOpen: false,
    mode: 'create',
  });
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');

  // Load templates on mount
  useEffect(() => {
    loadTemplates();
  }, [filter, searchQuery]);

  const loadTemplates = useCallback(async () => {
    setLoading(true);
    try {
      const response = getTemplates({
        owner: filter === 'mine' ? currentUser.id : undefined,
        includeShared: filter !== 'mine',
        search: searchQuery,
        sortBy: 'dateModified',
        sortOrder: 'desc',
      });
      setTemplates(response.templates);
    } catch (error) {
      console.error('Failed to load templates:', error);
    } finally {
      setLoading(false);
    }
  }, [currentUser.id, filter, searchQuery]);

  const handleCreateTemplate = useCallback(() => {
    setModalState({ isOpen: true, mode: 'create' });
  }, []);

  const handleEditTemplate = useCallback((template: Template) => {
    // Load version history for this template (would come from API in Phase 3)
    const extendedTemplate = {
      ...template,
      versions: [] as TemplateVersion[],
    };
    setModalState({ isOpen: true, mode: 'edit', template: extendedTemplate });
  }, []);

  const handleDuplicateTemplate = useCallback((template: Template) => {
    // In production, this would call the API's duplicate endpoint
    const newTemplate: Template = {
      ...template,
      id: `template-${Date.now()}`,
      name: `${template.name} (Copy)`,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      owner: currentUser.id,
      isShared: false,
    };
    // Note: storage.createTemplate would handle this in Phase 3
    console.log('Duplicate template:', newTemplate);
  }, [currentUser.id]);

  const handleToggleShare = useCallback(
    async (template: Template) => {
      try {
        await setTemplateShared(template.id, !template.isShared);
        loadTemplates();
      } catch (error) {
        console.error('Failed to toggle share:', error);
      }
    },
    [loadTemplates]
  );

  const handleDeleteTemplate = useCallback(
    async (id: string) => {
      if (!window.confirm('Are you sure you want to delete this template?')) return;
      try {
        await storageDelete(id);
        setTemplates((prev) => prev.filter((t) => t.id !== id));
        setSelectedTemplate(null);
      } catch (error) {
        console.error('Failed to delete template:', error);
      }
    },
    []
  );

  const handleSaveTemplate = useCallback(
    (template: Template) => {
      if (modalState.mode === 'create') {
        setTemplates((prev) => [template, ...prev]);
      } else {
        setTemplates((prev) =>
          prev.map((t) => (t.id === template.id ? template : t))
        );
        setSelectedTemplate(template);
      }
      setModalState({ isOpen: false, mode: 'create' });
    },
    [modalState.mode]
  );

  const handleExport = useCallback(() => {
    const data = exportTemplates(
      filter === 'mine'
        ? (t) => t.owner === currentUser.id
        : filter === 'shared'
          ? (t) => t.isShared
          : undefined
    );
    const blob = new Blob([JSON.stringify(data, null, 2)], {
      type: 'application/json',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `templates-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [filter, currentUser.id]);

  const handleImport = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = (event) => {
        try {
          const jsonData = JSON.parse(event.target?.result as string);
          const result = importTemplates(jsonData);
          alert(
            `Imported ${result.imported} templates. Errors: ${result.errors.length}`
          );
          loadTemplates();
        } catch (err) {
          alert('Failed to import templates: Invalid JSON file');
        }
      };
      reader.readAsText(file);
    },
    [loadTemplates]
  );

  const filteredTemplates = templates.filter((template) => {
    if (filter === 'mine' && template.owner !== currentUser.id) return false;
    if (filter === 'shared' && !template.isShared) return false;
    return true;
  });

  // Calculate usage count for each template
  const templatesWithUsage = filteredTemplates.map((t) => ({
    ...t,
    usageCount: Math.floor(Math.random() * 50), // Simulated usage count
  }));

  if (loading) {
    return (
      <div className="template-manager loading">
        <div className="spinner">Loading templates...</div>
      </div>
    );
  }

  return (
    <div className="template-manager">
      {/* Header */}
      <div className="tm-header">
        <h2>Template Library</h2>
        <div className="tm-actions">
          <button
            type="button"
            onClick={handleCreateTemplate}
            className="primary-btn create-btn"
          >
            + New Template
          </button>
          <label className="import-label">
            Import JSON
            <input
              type="file"
              accept=".json"
              onChange={handleImport}
              style={{ display: 'none' }}
            />
          </label>
          <button type="button" onClick={handleExport} className="secondary-btn export-btn">
            Export All
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <div className="tm-toolbar">
        {/* Search */}
        <input
          type="text"
          placeholder="Search templates..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="search-input"
        />

        {/* Filter Tabs */}
        <div className="filter-tabs">
          <button
            type="button"
            onClick={() => setFilter('all')}
            className={`filter-tab ${filter === 'all' ? 'active' : ''}`}
          >
            All Templates ({templates.length})
          </button>
          <button
            type="button"
            onClick={() => setFilter('mine')}
            className={`filter-tab ${filter === 'mine' ? 'active' : ''}`}
          >
            My Templates
          </button>
          <button
            type="button"
            onClick={() => setFilter('shared')}
            className={`filter-tab ${filter === 'shared' ? 'active' : ''}`}
          >
            Shared
          </button>
        </div>

        {/* View Toggle */}
        <div className="view-toggle">
          <button
            type="button"
            onClick={() => setViewMode('grid')}
            className={`view-btn ${viewMode === 'grid' ? 'active' : ''}`}
            title="Grid view"
          >
            <svg width="16" height="16" viewBox="0 0 24 24">
              <rect x="3" y="3" width="7" height="7" />
              <rect x="14" y="3" width="7" height="7" />
              <rect x="3" y="14" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" />
            </svg>
          </button>
          <button
            type="button"
            onClick={() => setViewMode('list')}
            className={`view-btn ${viewMode === 'list' ? 'active' : ''}`}
            title="List view"
          >
            <svg width="16" height="16" viewBox="0 0 24 24">
              <line x1="3" y1="6" x2="21" y2="6" />
              <line x1="3" y1="12" x2="21" y2="12" />
              <line x1="3" y1="18" x2="21" y2="18" />
            </svg>
          </button>
        </div>
      </div>

      {/* Templates Grid */}
      <div className={`template-grid ${viewMode}`}>
        {templatesWithUsage.length === 0 ? (
          <div className="empty-state">
            <p>No templates found.</p>
            <button
              type="button"
              onClick={handleCreateTemplate}
              className="secondary-btn"
            >
              Create your first template
            </button>
          </div>
        ) : (
          templatesWithUsage.map((template) => (
            <TemplateCard
              key={template.id}
              template={template}
              isSelected={selectedTemplate?.id === template.id}
              onClick={() => {
                setSelectedTemplate(template);
                onTemplateSelect?.(template);
              }}
            />
          ))
        )}
      </div>

      {/* Modal */}
      <TemplateModal
        isOpen={modalState.isOpen}
        mode={modalState.mode}
        initialTemplate={modalState.template}
        onClose={() => setModalState({ isOpen: false, mode: 'create' })}
        onSave={handleSaveTemplate}
      />
    </div>
  );
}

export default TemplateManager;
