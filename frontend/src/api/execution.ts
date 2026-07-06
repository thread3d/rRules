// frontend/src/api/execution.ts — Phase 28 execution API client (D-10).
//
// Typed bridge the TestRunner (Plan 28-05) calls. Six async functions, one per
// endpoint, mirroring the one-function-per-endpoint pattern of ./rules.ts:
//   - executeRule     → POST /v1/execute                 (D-08 Run button)
//   - batchExecute    → POST /v1/rules/batch-execute     (ROADMAP deliverable)
//   - getJobStatus    → GET  /v1/jobs/{job_id}           (D-07 Reload-then-Run poll)
//   - listJobs        → GET  /v1/jobs                     (ROADMAP deliverable)
//   - listEngines     → GET  /v1/engines                  (D-08 dropdown data source)
//   - reloadRules     → POST /v1/rules/reload            (D-07 Reload-then-Run kick-off)
//
// `reloadRules` is the 6th function — Critical Finding 3: D-10's original
// 5-function list omitted it, but D-07 "Reload then Run" needs it to kick off
// the async reload job before polling getJobStatus.
//
// All functions reuse the single shared apiClient from ./client (L-05 — no
// second axios instance, no new interceptors; the existing 401/auth interceptor
// covers execution.ts). Response bodies are typed via ./types so callers never
// see `any`. Types use snake_case keys matching backend serde (D-10).

import apiClient from './client'
import type {
  ExecuteRequest,
  ExecuteResponse,
  EnginesResponse,
  ReloadJob,
  BatchExecuteRequest,
  BatchExecuteResponse,
} from './types'

// Execute a rule set against the supplied facts. POST /v1/execute.
// (D-08: the TestRunner "Run" button's useMutation calls this.)
export async function executeRule(req: ExecuteRequest): Promise<ExecuteResponse> {
  const response = await apiClient.post<ExecuteResponse>('/execute', req)
  return response.data
}

// Execute multiple rule sets concurrently in one call. POST /v1/rules/batch-execute.
// (ROADMAP deliverable; no UI consumer in Phase 28 — exported for completeness.)
export async function batchExecute(req: BatchExecuteRequest): Promise<BatchExecuteResponse> {
  const response = await apiClient.post<BatchExecuteResponse>('/rules/batch-execute', req)
  return response.data
}

// Get the status of a reload job. GET /v1/jobs/{job_id}.
// (D-07: Reload-then-Run polls this until status is 'completed' or 'failed'.)
export async function getJobStatus(jobId: string): Promise<ReloadJob> {
  const response = await apiClient.get<ReloadJob>(`/jobs/${jobId}`)
  return response.data
}

// List all reload jobs. GET /v1/jobs.
// (Backend serializes `Vec<ReloadJob>` directly — no envelope to unwrap.)
export async function listJobs(): Promise<ReloadJob[]> {
  const response = await apiClient.get<ReloadJob[]>('/jobs')
  return response.data
}

// List available engines + the resolved default. GET /v1/engines.
// (D-08: the TestRunner engine dropdown's useQuery data source.)
export async function listEngines(): Promise<EnginesResponse> {
  const response = await apiClient.get<EnginesResponse>('/engines')
  return response.data
}

// Kick off a reload. POST /v1/rules/reload with { async_reload: <bool> }.
// (D-07: Reload-then-Run calls reloadRules({ async_reload: true }) → 202 +
// JobResponse { job_id, status }, then polls getJobStatus until terminal.)
// The 6th function — Critical Finding 3: D-10's original list omitted it.
export async function reloadRules(
  asyncReload = true,
): Promise<{ job_id: string; status: string }> {
  const response = await apiClient.post<{ job_id: string; status: string }>('/rules/reload', {
    async_reload: asyncReload,
  })
  return response.data
}