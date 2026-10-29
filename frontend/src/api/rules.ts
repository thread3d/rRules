import apiClient from './client'
import type {
  Rule,
} from '@/types/common'
import type {
  CreateRuleRequest,
  FilterRulesRequest,
  PaginatedRulesResponse,
  UpdateRuleRequest,
} from './types'


// The wire format is the API's (`rules.rs`): snake_case, `dsl_body`, `version`.
// The interface's `Rule` is a view model with camelCase fields and editor state
// (conditions, actions, hit policy). Nothing mapped between them, so every field
// the API did return arrived as `undefined` — dates included, which is why sorting
// by date compared `NaN`. Map at the boundary, once.
interface ApiRule {
  id: string
  name: string
  description?: string | null
  category?: string | null
  dsl_body?: string
  tags?: string[]
  status?: string
  version?: number
  executable?: boolean
  created_at?: string
  updated_at?: string
}

function toRule(rule: ApiRule): Rule {
  const known: Rule['status'][] = ['draft', 'active', 'inactive', 'pending-approval', 'deprecated']
  const status = known.includes(rule.status as Rule['status'])
    ? (rule.status as Rule['status'])
    : 'draft'
  return {
    id: rule.id,
    name: rule.name,
    description: rule.description ?? undefined,
    status,
    // Editor-only fields the list does not use yet; the API has no counterpart.
    hitPolicy: 'first',
    conditions: [],
    actions: [],
    ownerId: '',
    // The body the edit page shows and saves. Dropping it meant the page started blank and Save wrote the
    // blank back over the stored text.
    body: rule.dsl_body ?? '',
    executable: rule.executable ?? false,
    createdAt: rule.created_at ?? '',
    updatedAt: rule.updated_at ?? '',
    version: rule.version ?? 1,
  }
}

// Get all rules with optional filtering
export async function getRules(params?: FilterRulesRequest): Promise<Rule[]> {
  // `GET /v1/rules` answers `{total_count, rules[], page, page_size, total_pages}`
  // — the page envelope, not a `data` wrapper (specs/http-api.md, `rules.rs`).
  const response = await apiClient.get<PaginatedRulesResponse>('/rules', { params })
  return (response.data.rules ?? []).map(toRule)
}

// Get a single rule by ID
export async function getRule(id: string): Promise<Rule> {
  const response = await apiClient.get<ApiRule>(`/rules/${id}`)
  return toRule(response.data)
}

// Create a new rule
export async function createRule(rule: CreateRuleRequest): Promise<Rule> {
  const response = await apiClient.post<ApiRule>('/rules', rule)
  return toRule(response.data)
}

// Update an existing rule
export async function updateRule(id: string, rule: UpdateRuleRequest): Promise<Rule> {
  const response = await apiClient.put<ApiRule>(`/rules/${id}`, rule)
  return toRule(response.data)
}

// Delete a rule
export async function deleteRule(id: string): Promise<void> {
  await apiClient.delete(`/rules/${id}`)
}

// Change rule status.
//
// `PATCH /v1/rules/{id}/status` does not exist — the call answered 404, so status
// changes never worked from the interface. A status update is an update: `PUT
// /v1/rules/{id}` with the new status (`rules.rs`, `PUT /v1/rules/{id}`).
export async function changeRuleStatus(
  id: string,
  status: Rule['status']
): Promise<Rule> {
  const response = await apiClient.put<ApiRule>(`/rules/${id}`, { status })
  return toRule(response.data)
}

// Get rules by category
export async function getRulesByCategory(category: string): Promise<Rule[]> {
  const response = await apiClient.get<PaginatedRulesResponse>('/rules', {
    params: { category },
  })
  return (response.data.rules ?? []).map(toRule)
}

// Search rules by name.
//
// There is no `/rules/search` endpoint — the path was matched by `/rules/{id}` and
// answered 400 for a non-uuid id. The list endpoint has no name filter either, so
// search fetches the page and matches names locally; correctness over cleverness
// until the API grows a query parameter.
export async function searchRules(query: string): Promise<Rule[]> {
  const rules = await getRules({ page_size: 200 })
  const needle = query.trim().toLowerCase()
  if (!needle) return rules
  return rules.filter((rule) => rule.name.toLowerCase().includes(needle))
}
