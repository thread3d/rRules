//! Decision evaluation API (Phase 33–34).
//!
//! Wrappers for `POST /v1/decisions/{id}/evaluate`, `GET /v1/decisions/{id}`,
//! and the Phase 34 IR endpoint (`GET /v1/decisions/{id}/ir`).

import apiClient from './client'
import type { HitPolicy } from '@/features/rules/types'

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

/** What `GET /v1/decisions` lists — `engine_state::DmnSummary`, not a rich summary. */
export interface DecisionSummary {
  id: string
  name: string | null
  version: string | null
  updated_at: string
}

/**
 * What `GET /v1/decisions/{id}` returns: the stored document's metadata and size, **not** its table.
 * The decision table comes from `GET /v1/decisions/{id}/ir` (`DmnIrResponse` below).
 */
export interface DecisionDocument {
  id: string
  name: string
  content_length: number
  metadata: Record<string, unknown>
  created_at: string
  updated_at: string
}

/** The page envelope `GET /v1/decisions` sends — `list_decisions` in `api/src/v1/decisions.rs`. */
export interface DecisionListResponse {
  decisions: DecisionSummary[]
  total: number
  page: number
  per_page: number
  pages: number
}

export interface ReloadDmnResponse {
  status: string
  message: string
}

// =========================================================================
// IR types (Phase 34 — Plan 02)
// =========================================================================

/** Full parsed decision table IR returned by GET /v1/decisions/{id}/ir. */
export interface DmnIrResponse {
  decision_id: string
  name: string | null
  decision_table: {
    id: string | null
    name: string | null
    hit_policy: HitPolicy // canonical DMN spec string (e.g. "UNIQUE", "RULE ORDER")
    aggregation?: 'SUM' | 'MIN' | 'MAX' | 'COUNT' // only for COLLECT
    preferred_orientation?: string | null
    output_label?: string | null
    input_clauses: Array<{
      id?: string | null
      label?: string | null
      input_expression: string
      input_values?: unknown
      description?: unknown | null
    }>
    output_clauses: Array<{
      id?: string | null
      label?: string | null
      name?: string | null
      type_ref?: string | null
      output_values?: unknown
      default_output_entry?: string | null
      description?: unknown | null
    }>
    rules: Array<{
      id?: string | null
      description?: unknown | null
      input_entries: Array<{ text: { content: string; is_cdata: boolean } }>
      output_entries: Array<{ text: { content: string; is_cdata: boolean } }>
      annotation_entries: Array<{ text: { content: string; is_cdata: boolean } }>
    }>
  } | null
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
export async function listDecisions(): Promise<DecisionListResponse> {
  const response = await apiClient.get<DecisionListResponse>('/decisions')
  return response.data
}

/**
 * Get a specific decision's details.
 * GET /v1/decisions/{id}
 */
export async function getDecision(id: string): Promise<DecisionDocument> {
  const response = await apiClient.get<DecisionDocument>(`/decisions/${id}`)
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

/**
 * Get the parsed decision table IR for a specific decision.
 * GET /v1/decisions/{id}/ir
 *
 * Phase 34 (Plan 02): returns full DecisionTable IR as JSON including
 * hit_policy, aggregation, input_clauses with input_expression names,
 * output_clauses, and rules with entry arrays — needed for the frontend
 * to load a saved decision for editing.
 */
export async function getDecisionIr(id: string): Promise<DmnIrResponse> {
  const response = await apiClient.get<DmnIrResponse>(`/decisions/${id}/ir`)
  return response.data
}
