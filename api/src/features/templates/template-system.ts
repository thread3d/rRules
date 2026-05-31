// Template parameterization and class-based inheritance system

/**
 * Parameter definition for template substitution
 */
export interface Parameter {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'enum';
  defaultValue?: string;
  description?: string;
  options?: string[]; // For enum types
}

/**
 * AST node for rule template structure
 */
export interface TemplateASTNode {
  type: string;
  value?: string;
  children?: TemplateASTNode[];
}

/**
 * Rule template structure
 */
export interface Template {
  id: string;
  name: string;
  description: string;
  parameters: Parameter[];
  baseStructure: TemplateASTNode;
  createdAt: string;
  updatedAt: string;
  owner: string;
  isShared: boolean;
}

/**
 * Rule instance created from a template
 */
export interface TemplateInstance {
  id: string;
  templateId: string;
  values: Record<string, string>;
  instantiatedRule: any;
  parametersUsed: Parameter[];
  errors: string[];
}

/**
 * Validate parameter substitution before instantiation
 */
export function validateParameters(
  parameters: Parameter[],
  values: Record<string, string>
): { valid: boolean; missing: string[] } {
  const missing: string[] = [];
  let valid = true;

  for (const param of parameters) {
    if (!param.defaultValue && !values[param.name]) {
      missing.push(param.name);
      valid = false;
    }
  }

  return { valid, missing };
}

/**
 * Instantiate a template with user-provided values
 */
export function instantiateTemplate(
  template: Template,
  values: Record<string, string>
): TemplateInstance {
  const validation = validateParameters(template.parameters, values);

  // Clone the base structure to avoid modifying the original
  const clonedStructure = JSON.parse(JSON.stringify(template.baseStructure));

  // Replace parameter placeholders with user values
  const instantiatedRule = replacePlaceholders(
    clonedStructure,
    template.parameters,
    values
  );

  return {
    id: `instance-${Date.now()}`,
    templateId: template.id,
    values,
    instantiatedRule,
    parametersUsed: template.parameters,
    errors: validation.missing,
  };
}

/**
 * Replace parameter placeholders in AST structure
 */
function replacePlaceholders(
  node: TemplateASTNode,
  parameters: Parameter[],
  values: Record<string, string>
): any {
  if (node.value && typeof node.value === 'string') {
    let result = node.value;

    // Find all {parameter_name} patterns and replace them
    for (const param of parameters) {
      const pattern = new RegExp(`\\{${param.name}\\}`, 'g');
      result = result.replace(pattern, values[param.name] || '');
    }

    return result;
  }

  if (node.children) {
    return {
      ...node,
      children: node.children.map((child) =>
        replacePlaceholders(child, parameters, values)
      ),
    };
  }

  return node;
}

/**
 * Resolve parameter with default value
 */
export function resolveParameters(
  parameters: Parameter[],
  userValues: Record<string, string>
): Record<string, string> {
  const resolved: Record<string, string> = {};

  for (const param of parameters) {
    if (userValues[param.name] !== undefined && userValues[param.name] !== '') {
      resolved[param.name] = userValues[param.name];
    } else if (param.defaultValue) {
      resolved[param.name] = param.defaultValue;
    }
  }

  return resolved;
}

/**
 * Merge parent template with child overrides recursively
 */
export function mergeInheritance(
  parent: TemplateASTNode,
  childOverrides: Record<string, any>
): TemplateASTNode {
  const merged = { ...parent };

  for (const [key, value] of Object.entries(childOverrides)) {
    if (
      typeof value === 'object' &&
      !Array.isArray(value) &&
      value !== null &&
      key !== 'parameters'
    ) {
      // Recursively merge nested objects
      if (merged[key as keyof TemplateASTNode]) {
        merged[key] = mergeInheritance(
          merged[key as keyof TemplateASTNode],
          value
        );
      } else {
        merged[key] = value;
      }
    } else {
      // Direct override for primitives and arrays
      merged[key as keyof TemplateASTNode] = value;
    }
  }

  return merged;
}

/**
 * Parse template reference from rule definition
 */
export function parseTemplateReference(ruleText: string): string | null {
  const extendsPattern = /extends\s+["']([^"']+)["']/i;
  const match = ruleText.match(extendsPattern);
  return match ? match[1] : null;
}

/**
 * Detect parameter placeholders in template text
 */
export function detectParameters(templateText: string): Parameter[] {
  const paramPattern = /\{([^}]+)\}/g;
  const matches = new Set<string>();
  let match;

  while ((match = paramPattern.exec(templateText)) !== null) {
    matches.add(match[1]);
  }

  return Array.from(matches).map((name) => ({
    name,
    type: 'string',
    defaultValue: '',
  }));
}

/**
 * Extract parameters and their types from template
 */
export function extractParametersFromAST(
  node: TemplateASTNode,
  foundParams = new Set<string>()
): Parameter[] {
  if (node.value && typeof node.value === 'string') {
    const paramPattern = /\{([^}]+)\}/g;
    let match;
    while ((match = paramPattern.exec(node.value)) !== null) {
      foundParams.add(match[1]);
    }
  }

  if (node.children) {
    for (const child of node.children) {
      extractParametersFromAST(child, foundParams);
    }
  }

  return Array.from(foundParams).map((name) => ({
    name,
    type: 'string',
    defaultValue: '',
  }));
}

export default instantiateTemplate;
