//! Decision evaluation API (Phase 33–34).
//!
//! Wrappers for `POST /v1/decisions/{id}/evaluate`, `GET /v1/decisions/{id}`,
//! and the Phase 34 IR endpoint (`GET /v1/decisions/{id}/ir`).

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





// =========================================================================
// IR types (Phase 34 — Plan 02)
// =========================================================================


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
