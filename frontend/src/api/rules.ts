import apiClient from './client'
import type {
  Rule,
} from '@/types/common'
import type {
  FilterRulesRequest,
  CreateRuleRequest,
  UpdateRuleRequest,
  RulesResponse,
  RuleResponse,
  PaginatedRulesResponse,
} from './types'

// Get all rules with optional filtering
export async function getRules(params?: FilterRulesRequest): Promise<Rule[]> {
  const response = await apiClient.get<PaginatedRulesResponse>('/rules', { params })
  return response.data.data
}

// Get a single rule by ID
export async function getRule(id: string): Promise<Rule> {
  const response = await apiClient.get<RuleResponse>(`/rules/${id}`)
  return response.data.data
}

// Create a new rule
export async function createRule(rule: CreateRuleRequest): Promise<Rule> {
  const response = await apiClient.post<RuleResponse>('/rules', rule)
  return response.data.data
}

// Update an existing rule
export async function updateRule(id: string, rule: UpdateRuleRequest): Promise<Rule> {
  const response = await apiClient.put<RuleResponse>(`/rules/${id}`, rule)
  return response.data.data
}

// Delete a rule
export async function deleteRule(id: string): Promise<void> {
  await apiClient.delete(`/rules/${id}`)
}

// Change rule status
export async function changeRuleStatus(
  id: string,
  status: Rule['status']
): Promise<Rule> {
  const response = await apiClient.patch<RuleResponse>(`/rules/${id}/status`, { status })
  return response.data.data
}

// Get rules by category
export async function getRulesByCategory(category: string): Promise<Rule[]> {
  const response = await apiClient.get<RulesResponse>('/rules', { params: { category } })
  return response.data.data
}

// Search rules by query
export async function searchRules(query: string): Promise<Rule[]> {
  const response = await apiClient.get<RulesResponse>('/rules/search', { params: { q: query } })
  return response.data.data
}
