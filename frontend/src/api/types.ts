
import type { Rule } from '@/types/common'

// API Request/Response types

// Mirrors `CreateRuleRequest` in `api/src/v1/rules.rs`. The body is stored as text:
// the API validates that it is non-empty and never parses it.
export interface CreateRuleRequest {
  name: string
  description?: string
  dsl_body: string
  tags?: string[]
  salience?: number
  status?: 'draft' | 'active'
  /**
   * The engine rule this document is. When present the API validates it with the engine and writes it into
   * the rules directory, so the rule is executable after the next reload; without it the document is a
   * draft no engine can run (`executable: false`). Build it with `api/rule-builder.ts`.
   */
  rule?: unknown
}

export interface UpdateRuleRequest extends Partial<CreateRuleRequest> {}

export interface RulesResponse {
  success: boolean
  data: Rule[]
}

export interface RuleResponse {
  success: boolean
  data: Rule
}

// `GET /v1/rules` answers the page envelope directly (see `specs/http-api.md`).
export interface PaginatedRulesResponse {
  total_count: number
  rules: Rule[]
  page: number
  page_size: number
  total_pages: number
}

// Mirrors `RuleListRequest` in `api/src/v1/rules.rs`. The client previously sent
// `perPage`, `owner` and `search`, which the server ignores.
export interface FilterRulesRequest {
  page?: number
  page_size?: number
  category?: string
  status?: string
  tag?: string
  sort_by?: string
  sort_order?: 'asc' | 'desc'
}

// Auth API types

export interface LoginRequest {
  email: string
  password: string
}

export interface LoginResponse {
  success: boolean
  token: string
  user: {
    id: string
    email: string
    displayName: string
    role: string
  }
}

export interface RegisterRequest {
  email: string
  password: string
  displayName: string
}

export interface RegisterResponse extends LoginResponse {}

// Approval types

export interface ApprovalState {
  Draft: string
  Submitted: { submitted_at: string }
  InReview: { started_at: string, current_tier: number }
  Approved: { approved_by: string[], completed_at: string }
  Rejected: { rejected_by: string, reason?: string, completed_at: string }
  Escalated: { escalated_to_tier: number, escalated_at: string }
}

export interface ApprovalRequest {
  id: string
  rule_id: string
  workflow_id: string
  maker_id: string
  state: string
  reviewers: string[]
  required_approvals: number
  approved_by: string[]
  created_at: string
}

export interface SubmitApprovalRequest {
  workflow_id?: string
  maker_id?: string
}

export interface ApprovalActionRequest {
  approver_id?: string
  reason?: string
}

export interface ApprovalsResponse {
  total_count: number
  approvals: Array<{
    id: string
    rule_id: string
    maker_id: string
    state: string
    created_at: string
  }>
}

// ---------------------------------------------------------------------------
// Phase 28 — Execution / Trace / Engine / Job types (D-10, EXEC-04)
//
// These interfaces mirror the backend `serde::Serialize`/`Deserialize` shapes
// in `api/src/v1/execution.rs` EXACTLY — snake_case keys, matching
// nullability, no camelCase transform. The existing `ApprovalState` /
// `ApprovalsResponse` block above sets the precedent (backend serde is
// snake_case; the frontend does not rename keys). Plan 28-05's TestRunner
// renders these responses directly, so a key mismatch surfaces as a runtime
// shape error, not a silent transform. See D-10 (snake_case mirroring) and
// EXEC-04 (trace types) for the decision trail.
// ---------------------------------------------------------------------------

/** Request body for `POST /v1/execute` (execution.rs ExecuteRequest :21-35). */
export interface ExecuteRequest {
  session_id?: string | null
  facts: Record<string, unknown>
  rule_filter?: {
    name_pattern?: string
    // No `categories`: the API refuses that field with a 400, because the executing ruleset carries no
    // category. Declaring it here invited a caller to send something that can only be rejected.
  } | null
  engine?: string | null
  /**
   * Opt-in inference trace (Plan 28-03 adds this to the backend as
   * `#[serde(default)] pub trace: Option<bool>`). Off-by-default so SC-3
   * latency neutrality holds; `null`/omitted ⇒ backend treats as `false`.
   */
  trace?: boolean | null
}

/** Response from `POST /v1/execute` (execution.rs ExecuteResponse :47-73). */
export interface ExecuteResponse {
  /** Rules the filter selected from the engine's rule set; 0 means it matched none. */
  rules_matched: number
  execution_id: string
  rules_fired: number
  rules_not_fired: string[]
  facts_after: Record<string, unknown>
  inference_trace?: InferenceTrace | null
  engine_generation: number
  session_id?: string | null
}

/** Complete inference trace (execution.rs InferenceTrace :188-194). */
export interface InferenceTrace {
  entries: InferenceTraceEntry[]
  summary?: TraceSummary | null
}

/** Single trace entry emitted at the moment a rule fires (execution.rs :197-205). */
export interface InferenceTraceEntry {
  timestamp: string
  event_type: string
  rule_name?: string | null
  data?: unknown | null
}

/** Summary statistics for an inference trace (execution.rs TraceSummary :208-212). */
export interface TraceSummary {
  total_duration_ms: number
  facts_matched: number
}

/** Response from `GET /v1/engines` (Plan 28-03 / D-03). */
export interface EnginesResponse {
  engines: string[]
  default: string
}

/** Reload job row (execution.rs ReloadJob :382-408, serialized with skip-None). */
export interface ReloadJob {
  job_id: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  result?: unknown
  error?: string | null
}

/** Request body for `POST /v1/rules/batch-execute` (execution.rs :462-471). */
export interface BatchExecuteRequest {
  requests: ExecuteRequest[]
  max_concurrency?: number | null
}

/** Per-item result inside a BatchExecuteResponse (execution.rs BatchItemResult :474-481). */
export interface BatchItemResult {
  success: boolean
  response?: ExecuteResponse | null
  error?: string | null
}

/** Response from `POST /v1/rules/batch-execute` (execution.rs :484-494). */
export interface BatchExecuteResponse {
  results: BatchItemResult[]
  total_fired: number
  succeeded: number
  failed: number
}
