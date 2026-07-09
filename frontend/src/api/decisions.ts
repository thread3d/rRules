//! Decision evaluation API (Phase 33).
//!
//! Wrappers for `POST /v1/decisions/{id}/evaluate` and related endpoints.

import apiClient from './client'

// =========================================================================
// Types
// =========================================================================

export interface EvaluateDecisionRequest {
  inputs: Record<string, unknown>
  trace?: boolean
}

export interface TraceResponse {
  decision_id: string
  event_count: number
  duration_ms: number
}

export interface EvaluateDecisionResponse {
  decision_id: string
  outputs: Record<string, unknown>
  rules_fired: number
  duration_ms: number
  trace?: TraceResponse
}

export interface DecisionSummary {
  id: string
  name: string | null
  description: string | null
  hit_policy: string
}

export interface DecisionDetail {
  id: string
  name: string | null
  description: string | null
  question: string | null
  decision_table: {
    id: string | null
    name: string | null
    hit_policy: string
    input_clauses: Array<{ label?: string; input_expression: string }>
    output_clauses: Array<{ name?: string; type_ref?: string }>
  } | null
}

export interface ReloadDmnResponse {
  status: string
  message: string
}

// =========================================================================
// API functions
// =========================================================================

/**
 * Evaluate a decision with the given inputs.
 * POST /v1/decisions/{id}/evaluate
 */
export async function evaluateDecision(
  id: string,
  request: EvaluateDecisionRequest
): Promise<EvaluateDecisionResponse> {
  const response = await apiClient.post<EvaluateDecisionResponse>(
    `/decisions/${id}/evaluate`,
    request
  )
  return response.data
}

/**
 * List all decisions in the current DMN model.
 * GET /v1/decisions
 */
export async function listDecisions(): Promise<{
  decisions: DecisionSummary[]
  model_version: number
}> {
  const response = await apiClient.get<{
    decisions: DecisionSummary[]
    model_version: number
  }>('/decisions')
  return response.data
}

/**
 * Get a specific decision's details.
 * GET /v1/decisions/{id}
 */
export async function getDecision(id: string): Promise<DecisionDetail> {
  const response = await apiClient.get<DecisionDetail>(`/decisions/${id}`)
  return response.data
}

/**
 * Reload a DMN model from XML content.
 * POST /v1/decisions/reload
 */
export async function reloadDmn(dmnContent: string): Promise<ReloadDmnResponse> {
  const response = await apiClient.post<ReloadDmnResponse>(
    '/decisions/reload',
    { dmn_content: dmnContent }
  )
  return response.data
}
