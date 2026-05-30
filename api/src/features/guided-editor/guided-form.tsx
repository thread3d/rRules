// Form-based interface for business users with step-by-step guided editing

import React, { useState, useEffect, useCallback } from 'react';

import { Accordion, AccordionSection } from '../components/hybrid-accordion';
import {
  VocabularyToggle,
  useVocabularyContext,
  FormLabel,
  VOCABULARY,
} from '../adaptive-vocabulary/vocabulary-toggle';
import { createRuleLinter, SyntaxError } from '../validation/realtime-validator';

/**
 * Common operators for rule conditions
 */
export const OPERATORS = ['>', '<', '==', '!=', '>=', '<='];

/**
 * Rule form state
 */
interface RuleFormData {
  name: string;
  description: string;
  entity: string;
  conditions: Array<{
    field: string;
    operator: string;
    value: string;
  }>;
  actions: Array<{
    type: string;
    params: Record<string, string>;
  }>;
}

/**
 * Smart defaults utility for auto-populating form fields
 */
export const SmartDefaults = {
  generateRuleName: (formData: Partial<RuleFormData>): string => {
    if (!formData.entity || !formData.conditions?.[0]?.field) {
      return `Rule ${new Date().toISOString().slice(0, 10)}`;
    }
    return `Rule for ${formData.entity} - ${formData.conditions[0].field}`;
  },

  generateDescription: (formData: Partial<RuleFormData>): string => {
    if (!formData.actions?.[0]?.type) {
      return '';
    }
    const action = formData.actions[0].type;
    return `Automatically applies ${action} when conditions are met`;
  },

  suggestOperator: (fieldType?: string): string => {
    switch (fieldType) {
      case 'number':
        return '>';
      case 'text':
        return '==';
      case 'date':
        return '>';
      default:
        return '==';
    }
  },
};

/**
 * Create rule object from form data
 */
export function createRuleFromFormData(formData: RuleFormData): any {
  const conditionString = formData.conditions
    .map((c) => `${c.field} ${c.operator} ${c.value}`)
    .join(' AND ');

  return {
    id: `rule-${Date.now()}`,
    name: formData.name,
    description: formData.description,
    condition: conditionString,
    actions: formData.actions.map((a) => a.type),
    enabled: true,
  };
}

/**
 * Props for GuidedForm
 */
interface GuidedFormProps {
  initialData?: Partial<RuleFormData>;
  onSave?: (rule: any) => void;
}

/**
 * GuidedForm - Form-based interface for business users
 */
export function GuidedForm({ initialData = {}, onSave }: GuidedFormProps) {
  const [step, setStep] = useState(1);
  const [formData, setFormData] = useState<RuleFormData>({
    name: SmartDefaults.generateRuleName(initialData),
    description: SmartDefaults.generateDescription(initialData),
    entity: initialData.entity || '',
    conditions: initialData.conditions?.length
      ? initialData.conditions
      : [{ field: '', operator: '>', value: '' }],
    actions: initialData.actions?.length
      ? initialData.actions
      : [{ type: '', params: {} }],
  });
  const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});
  const { translate } = useVocabularyContext();

  // Auto-update name/description when conditions/actions change
  useEffect(() => {
    if (initialData.entity && !formData.name.includes(formData.entity)) {
      setFormData((prev) => ({
        ...prev,
        name: SmartDefaults.generateRuleName(prev),
      }));
    }
  }, [initialData.entity]);

  // Validation per step
  const validateStep = useCallback(
    (currentStep: number): boolean => {
      const errors: Record<string, string> = {};

      if (currentStep === 1) {
        // Entity selection
        if (!formData.entity.trim()) {
          errors.entity = 'Please select a business entity';
        }
      } else if (currentStep === 2) {
        // Condition definition
        formData.conditions.forEach((condition, index) => {
          if (!condition.field.trim()) {
            errors[`condition-${index}-field`] = 'Field is required';
          }
          if (!condition.operator) {
            errors[`condition-${index}-operator`] = 'Operator is required';
          }
          if (!condition.value.trim()) {
            errors[`condition-${index}-value`] = 'Value is required';
          }
        });
      } else if (currentStep === 3) {
        // Action configuration
        formData.actions.forEach((action, index) => {
          if (!action.type.trim()) {
            errors[`action-${index}-type`] = 'Action type is required';
          }
        });
      }

      setValidationErrors(errors);
      return Object.keys(errors).length === 0;
    },
    [formData]
  );

  const handleNext = useCallback(() => {
    if (validateStep(step)) {
      setStep((prev) => Math.min(prev + 1, 4));
    }
  }, [step, validateStep]);

  const handleBack = useCallback(() => {
    setStep((prev) => Math.max(prev - 1, 1));
  }, []);

  // Step 1: Entity Selection
  const renderEntitySelection = () => (
    <div className="form-step">
      <h3>1. Select Business Entity</h3>
      <p>Select the type of entity this rule applies to.</p>

      <FormLabel htmlFor="entity" term="entity" required />
      <select
        id="entity"
        value={formData.entity}
        onChange={(e) =>
          setFormData((prev) => ({ ...prev, entity: e.target.value }))
        }
        className={`form-control ${validationErrors.entity ? 'error' : ''}`}
      >
        <option value="">-- Select Entity --</option>
        <option value="customer">Customer</option>
        <option value="order">Order</option>
        <option value="product">Product</option>
        <option value="account">Account</option>
      </select>
      {validationErrors.entity && (
        <span className="error-message">{validationErrors.entity}</span>
      )}

      <FormLabel htmlFor="rule-name" term="ruleName" required />
      <input
        type="text"
        id="rule-name"
        value={formData.name}
        onChange={(e) =>
          setFormData((prev) => ({ ...prev, name: e.target.value }))
        }
        className="form-control"
        placeholder={translate('ruleName')}
      />

      <FormLabel htmlFor="description" term="description" />
      <textarea
        id="description"
        value={formData.description}
        onChange={(e) =>
          setFormData((prev) => ({ ...prev, description: e.target.value }))
        }
        className="form-control"
        placeholder={translate('description')}
      />
    </div>
  );

  // Step 2: Conditions
  const renderConditionDefinition = () => (
    <div className="form-step">
      <h3>2. Define Conditions</h3>
      <p>Specify when this rule should be triggered.</p>

      {formData.conditions.map((condition, index) => (
        <div key={index} className="condition-row">
          <FormLabel htmlFor={`field-${index}`} term="field" />
          <select
            id={`field-${index}`}
            value={condition.field}
            onChange={(e) => {
              const newConditions = [...formData.conditions];
              newConditions[index] = {
                ...condition,
                field: e.target.value,
                operator: SmartDefaults.suggestOperator(
                  getFieldType(e.target.value)
                ),
              };
              setFormData((prev) => ({ ...prev, conditions: newConditions }));
            }}
            className={`form-control ${validationErrors[`condition-${index}-field`] ? 'error' : ''}`}
          >
            <option value="">-- Select Field --</option>
            <optgroup label="Customer">
              <option value="annual_spend">Annual Spend</option>
              <option value="join_date">Join Date</option>
              <option value="tier">Tier</option>
            </optgroup>
            <optgroup label="Order">
              <option value="total_amount">Total Amount</option>
              <option value="quantity">Quantity</option>
            </optgroup>
          </select>

          <FormLabel htmlFor={`operator-${index}`} term="operator" />
          <select
            id={`operator-${index}`}
            value={condition.operator}
            onChange={(e) => {
              const newConditions = [...formData.conditions];
              newConditions[index] = {
                ...condition,
                operator: e.target.value,
              };
              setFormData((prev) => ({ ...prev, conditions: newConditions }));
            }}
            className={`form-control ${validationErrors[`condition-${index}-operator`] ? 'error' : ''}`}
          >
            {OPERATORS.map((op) => (
              <option key={op} value={op}>
                {translate('operator')}
              </option>
            ))}
          </select>

          <FormLabel htmlFor={`value-${index}`} term="value" />
          <input
            type="text"
            id={`value-${index}`}
            value={condition.value}
            onChange={(e) => {
              const newConditions = [...formData.conditions];
              newConditions[index] = {
                ...condition,
                value: e.target.value,
              };
              setFormData((prev) => ({ ...prev, conditions: newConditions }));
            }}
            className={`form-control ${validationErrors[`condition-${index}-value`] ? 'error' : ''}`}
          />

          {formData.conditions.length > 1 && (
            <button
              type="button"
              onClick={() =>
                setFormData((prev) => ({
                  ...prev,
                  conditions: prev.conditions.filter((_, i) => i !== index),
                }))
              }
              className="remove-condition-btn"
            >
              x
            </button>
          )}
        </div>
      ))}

      <button
        type="button"
        onClick={() =>
          setFormData((prev) => ({
            ...prev,
            conditions: [
              ...prev.conditions,
              { field: '', operator: '>', value: '' },
            ],
          }))
        }
        className="add-condition-btn"
      >
        + Add Condition
      </button>
    </div>
  );

  // Step 3: Actions
  const renderActionConfiguration = () => (
    <div className="form-step">
      <h3>3. Configure Actions</h3>
      <p>Specify what should happen when the rule triggers.</p>

      {formData.actions.map((action, index) => (
        <div key={index} className="action-row">
          <FormLabel htmlFor={`action-type-${index}`} term="action" />
          <select
            id={`action-type-${index}`}
            value={action.type}
            onChange={(e) => {
              const newActions = [...formData.actions];
              newActions[index] = { ...action, type: e.target.value };
              setFormData((prev) => ({ ...prev, actions: newActions }));
            }}
            className={`form-control ${validationErrors[`action-${index}-type`] ? 'error' : ''}`}
          >
            <option value="">-- Select Action --</option>
            <option value="apply_discount">Apply Discount</option>
            <option value="add_tag">Add Tag</option>
            <option value="send_email">Send Email</option>
            <option value="create_task">Create Task</option>
          </select>

          {action.type === 'apply_discount' && (
            <div className="action-params">
              <FormLabel htmlFor={`discount-${index}`} term="value" />
              <input
                type="number"
                id={`discount-${index}`}
                value={action.params.discount || ''}
                onChange={(e) => {
                  const newActions = [...formData.actions];
                  newActions[index] = {
                    ...action,
                    params: { ...action.params, discount: e.target.value },
                  };
                  setFormData((prev) => ({ ...prev, actions: newActions }));
                }}
                placeholder="15"
              />
              <span>%</span>
            </div>
          )}

          <button
            type="button"
            onClick={() =>
              setFormData((prev) => ({
                ...prev,
                actions: prev.actions.filter((_, i) => i !== index),
              }))
            }
            className="remove-action-btn"
          >
            x
          </button>
        </div>
      ))}

      <button
        type="button"
        onClick={() =>
          setFormData((prev) => ({
            ...prev,
            actions: [...prev.actions, { type: '', params: {} }],
          }))
        }
        className="add-action-btn"
      >
        + Add Action
      </button>
    </div>
  );

  // Step 4: Review
  const renderReview = () => (
    <div className="form-step review-section">
      <h3>4. Review Your Rule</h3>
      <p>Please review your rule configuration before saving.</p>

      <div className="rule-summary">
        <div className="summary-item">
          <span className="label">Rule Name:</span>
          <span className="value">{formData.name}</span>
        </div>
        {formData.description && (
          <div className="summary-item">
            <span className="label">Description:</span>
            <span className="value">{formData.description}</span>
          </div>
        )}
        <div className="summary-item">
          <span className="label">Entity:</span>
          <span className="value">{translate(formData.entity)}</span>
        </div>
        <div className="summary-item conditions">
          <span className="label">Conditions:</span>
          <ul>
            {formData.conditions.map((c, i) => (
              <li key={i}>{c.field} {c.operator} {c.value}</li>
            ))}
          </ul>
        </div>
        <div className="summary-item actions">
          <span className="label">Actions:</span>
          <ul>
            {formData.actions.map((a, i) => (
              <li key={i}>{a.type}</li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );

  // Helper function to get field type
  const getFieldType = (field: string): string | undefined => {
    if (field.includes('amount') || field.includes('spend')) return 'number';
    if (field.includes('date')) return 'date';
    return 'text';
  };

  // Handle form save
  const handleSave = useCallback(() => {
    if (validateStep(4)) {
      const rule = createRuleFromFormData(formData);
      onSave?.(rule);
    }
  }, [formData, onSave, validateStep]);

  return (
    <div className="guided-form">
      {/* Vocabulary Toggle */}
      <div className="form-header">
        <h1>{translate('ruleName')}</h1>
        <VocabularyToggle />
      </div>

      <Accordion className="guided-accordion">
        <AccordionSection
          id="step-1"
          title={translate('entity')}
          isCompleted={step >= 2}
          requiredFieldsFilled={!validationErrors.entity}
        >
          {renderEntitySelection()}
          <div className="form-actions">
            <button
              type="button"
              onClick={handleNext}
              disabled={!formData.entity.trim()}
            >
              Next: Conditions
            </button>
          </div>
        </AccordionSection>

        <AccordionSection
          id="step-2"
          title={translate('condition')}
          isCompleted={step >= 3}
          requiredFieldsFilled={
            !Object.keys(validationErrors).some((k) =>
              k.startsWith('condition-')
            )
          }
        >
          {renderConditionDefinition()}
          <div className="form-actions">
            <button type="button" onClick={handleBack}>Back</button>
            <button
              type="button"
              onClick={handleNext}
              disabled={
                Object.keys(validationErrors).some((k) =>
                  k.startsWith('condition-')
                )
              }
            >
              Next: Actions
            </button>
          </div>
        </AccordionSection>

        <AccordionSection
          id="step-3"
          title={translate('action')}
          isCompleted={step >= 4}
          requiredFieldsFilled={
            !Object.keys(validationErrors).some((k) =>
              k.startsWith('action-')
            )
          }
        >
          {renderActionConfiguration()}
          <div className="form-actions">
            <button type="button" onClick={handleBack}>Back</button>
            <button
              type="button"
              onClick={handleNext}
              disabled={
                Object.keys(validationErrors).some((k) =>
                  k.startsWith('action-')
                )
              }
            >
              Next: Review
            </button>
          </div>
        </AccordionSection>

        <AccordionSection id="step-4" title="Review">
          {renderReview()}
          <div className="form-actions">
            <button type="button" onClick={handleBack}>Back</button>
            <button type="button" onClick={handleSave} className="save-btn">
              Save Rule
            </button>
          </div>
        </AccordionSection>
      </Accordion>
    </div>
  );
}

export default GuidedForm;
