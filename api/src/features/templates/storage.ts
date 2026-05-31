// Hybrid storage layer for templates with user-private/shared models

import { Template } from './template-system';

/**
 * User identity (would come from auth system in Phase 3)
 */
export interface User {
  id: string;
  name: string;
  email: string;
}

/**
 * Storage API responses
 */
export interface TemplatesResponse {
  templates: Template[];
  totalCount: number;
  hasMore: boolean;
}

/**
 * Local storage key for template cache
 */
const STORAGE_KEY = 'rRulesTemplates';
const USER_ID = 'currentUser';

// In-memory store (in production, this would be Phase 3 API)
let templatesStore: Template[] = [];
let currentUser: User | null = null;

/**
 * Set current user for access control
 */
export function setCurrentUser(user: User): void {
  currentUser = user;
  loadFromCache();
}

/**
 * Get current user
 */
export function getCurrentUser(): User | null {
  return currentUser;
}

/**
 * Load templates from localStorage cache
 */
function loadFromCache(): void {
  const cached = localStorage.getItem(STORAGE_KEY);
  if (cached) {
    try {
      templatesStore = JSON.parse(cached);
    } catch (e) {
      console.error('Failed to parse cached templates:', e);
      templatesStore = [];
    }
  }
}

/**
 * Save templates to localStorage cache
 */
function saveToCache(): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(templatesStore));
}

/**
 * Get all accessible templates (user-private + shared)
 */
export function getTemplates(options: {
  owner?: string;
  includeShared?: boolean;
  search?: string;
  sortBy?: 'name' | 'dateModified' | 'usageCount';
  sortOrder?: 'asc' | 'desc';
  page?: number;
  pageSize?: number;
} = {}): TemplatesResponse {
  const {
    owner,
    includeShared = true,
    search = '',
    sortBy = 'dateModified',
    sortOrder = 'desc',
    page = 1,
    pageSize = 50,
  } = options;

  let filtered = templatesStore.filter((template) => {
    // Ownership filter
    if (owner && template.owner !== owner) {
      return false;
    }

    // Shared access check
    if (!includeShared && !template.isShared && template.owner !== currentUser?.id) {
      return false;
    }

    // Search filter
    if (search) {
      const searchLower = search.toLowerCase();
      return (
        template.name.toLowerCase().includes(searchLower) ||
        template.description.toLowerCase().includes(searchLower)
      );
    }

    return true;
  });

  // Sort
  filtered.sort((a, b) => {
    let comparison = 0;
    switch (sortBy) {
      case 'name':
        comparison = a.name.localeCompare(b.name);
        break;
      case 'dateModified':
        comparison = new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
        break;
      case 'usageCount':
        comparison = (a as any).usageCount - (b as any).usageCount;
        break;
    }
    return sortOrder === 'desc' ? -comparison : comparison;
  });

  // Pagination
  const startIndex = (page - 1) * pageSize;
  const paginated = filtered.slice(startIndex, startIndex + pageSize);

  return {
    templates: paginated,
    totalCount: filtered.length,
    hasMore: startIndex + pageSize < filtered.length,
  };
}

/**
 * Create a new template
 */
export function createTemplate(templateData: Omit<Template, 'id' | 'createdAt' | 'updatedAt'>): Template {
  const newTemplate: Template = {
    ...templateData,
    id: `template-${Date.now()}`,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  templatesStore.push(newTemplate);
  saveToCache();

  return newTemplate;
}

/**
 * Update an existing template
 */
export function updateTemplate(id: string, updates: Partial<Template>): Template | null {
  const index = templatesStore.findIndex((t) => t.id === id);

  if (index === -1) {
    return null;
  }

  // Access control check
  if (templatesStore[index].owner !== currentUser?.id && !currentUser?.isAdmin) {
    throw new Error('Access denied: cannot modify template owned by another user');
  }

  templatesStore[index] = {
    ...templatesStore[index],
    ...updates,
    updatedAt: new Date().toISOString(),
  };

  saveToCache();
  return templatesStore[index];
}

/**
 * Delete a template
 */
export function deleteTemplate(id: string): boolean {
  const index = templatesStore.findIndex((t) => t.id === id);

  if (index === -1) {
    return false;
  }

  // Access control check
  if (templatesStore[index].owner !== currentUser?.id && !currentUser?.isAdmin) {
    throw new Error('Access denied: cannot delete template owned by another user');
  }

  templatesStore.splice(index, 1);
  saveToCache();

  return true;
}

/**
 * Get a single template by ID
 */
export function getTemplateById(id: string): Template | undefined {
  return templatesStore.find((t) => t.id === id);
}

/**
 * Duplicate a template (for shared templates)
 */
export function duplicateTemplate(
  id: string,
  newName?: string
): Template | null {
  const original = templatesStore.find((t) => t.id === id);

  if (!original) {
    return null;
  }

  // Check access - can only duplicate shared or owned templates
  if (original.owner !== currentUser?.id && !original.isShared) {
    throw new Error('Access denied: cannot duplicate this template');
  }

  const newTemplate: Template = {
    ...original,
    id: `template-${Date.now()}`,
    name: newName || `${original.name} (Copy)`,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    owner: currentUser?.id || 'unknown',
    isShared: false, // Duplicates are always user-private
  };

  templatesStore.push(newTemplate);
  saveToCache();

  return newTemplate;
}

/**
 * Search templates by name or description
 */
export function searchTemplates(query: string): Template[] {
  const lowerQuery = query.toLowerCase();
  return templatesStore.filter(
    (t) =>
      t.name.toLowerCase().includes(lowerQuery) ||
      t.description.toLowerCase().includes(lowerQuery)
  );
}

/**
 * Filter templates by ownership
 */
export function getMyTemplates(): Template[] {
  if (!currentUser) {
    return [];
  }
  return templatesStore.filter((t) => t.owner === currentUser.id);
}

export function getSharedTemplates(): Template[] {
  return templatesStore.filter((t) => t.isShared && !t.owner);
}

/**
 * Toggle template sharing status
 */
export function setTemplateShared(id: string, isShared: boolean): Template | null {
  const index = templatesStore.findIndex((t) => t.id === id);

  if (index === -1) {
    return null;
  }

  // Only owner can share/unshare
  if (templatesStore[index].owner !== currentUser?.id && !currentUser?.isAdmin) {
    throw new Error('Access denied: only template owner can change sharing status');
  }

  templatesStore[index] = {
    ...templatesStore[index],
    isShared,
    updatedAt: new Date().toISOString(),
  };

  saveToCache();
  return templatesStore[index];
}

/**
 * Import templates from JSON
 */
export function importTemplates(jsonData: any): { imported: number; errors: string[] } {
  const errors: string[] = [];
  let imported = 0;

  if (!Array.isArray(jsonData)) {
    jsonData = [jsonData];
  }

  for (const data of jsonData) {
    try {
      // Basic validation
      if (
        !data.name ||
        !data.parameters ||
        !data.baseStructure ||
        typeof data.isShared !== 'boolean'
      ) {
        errors.push(`Invalid template structure: ${JSON.stringify(data).substring(0, 50)}`);
        continue;
      }

      const newTemplate: Template = {
        id: `template-${Date.now()}-${imported}`,
        name: data.name,
        description: data.description || '',
        parameters: data.parameters,
        baseStructure: data.baseStructure,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        owner: currentUser?.id || 'unknown',
        isShared: data.isShared,
      };

      templatesStore.push(newTemplate);
      imported++;
    } catch (e) {
      errors.push(`Failed to import template: ${String(e)}`);
    }
  }

  saveToCache();
  return { imported, errors };
}

/**
 * Export templates to JSON
 */
export function exportTemplates(filter?: (t: Template) => boolean): any[] {
  let result = templatesStore;

  if (filter) {
    result = result.filter(filter);
  }

  return result.map((t) => ({
    name: t.name,
    description: t.description,
    parameters: t.parameters,
    baseStructure: t.baseStructure,
    isShared: t.isShared,
  }));
}

export { templatesStore };
