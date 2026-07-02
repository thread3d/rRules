import { useCallback, useEffect, useState } from 'react'
import apiClient from '@/api/client'

// ---------------------------------------------------------------------------
// Types & adapters (Pitfall 4: backend BranchResponse has no `id`; commit_sha
// vs lastCommitSha; BranchListResponse wraps { total_count, default_branch,
// branches[] }).
// ---------------------------------------------------------------------------

/** Backend GET /v1/branches item. */
interface BackendBranchResponse {
  name: string
  is_default: boolean
  commit_sha: string
  last_commit_message: string
  created_at: string
}

/** Backend GET /v1/branches envelope. */
interface BackendBranchListResponse {
  total_count: number
  default_branch: string | null
  branches: BackendBranchResponse[]
}

/** Frontend Branch shape. Backend has no `id`, so `name` doubles as the id. */
interface Branch {
  id: string
  name: string
  isDefault: boolean
  createdAt: string
  lastCommitSha?: string
  lastCommitMessage?: string
}

/** Map a backend branch into the frontend shape (P-6 backstop: no `undefined`). */
function mapBackendBranch(b: BackendBranchResponse): Branch {
  return {
    id: b.name,
    name: b.name,
    isDefault: Boolean(b.is_default),
    createdAt: b.created_at,
    lastCommitSha: b.commit_sha || undefined,
    lastCommitMessage: b.last_commit_message || undefined,
  }
}

/** Backend POST /v1/branches/{name}/merge 409 payload. */
interface MergeConflict {
  path: string
  old_version?: string | null
  base_version?: string | null
  new_version?: string | null
}

/** Backend GET /v1/rules/{id}/git-log response. */
interface GitLogResponse {
  branch: string
  total_commits: number
  commits: GitCommitEntry[]
}

interface GitCommitEntry {
  sha: string
  message: string
  author_name: string
  committed_at: string
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function BranchManager() {
  const [branches, setBranches] = useState<Branch[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [mergeTarget, setMergeTarget] = useState<Branch | null>(null)
  const [conflicts, setConflicts] = useState<MergeConflict[] | null>(null)
  const [mergeError, setMergeError] = useState<string | null>(null)
  const [diffOpen, setDiffOpen] = useState(false)

  const fetchBranches = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const response = await apiClient.get<BackendBranchListResponse>('/branches')
      const list = response.data?.branches ?? []
      setBranches(list.map(mapBackendBranch))
    } catch (err) {
      setError(errorMessage(err, 'Failed to load branches'))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchBranches()
  }, [fetchBranches])

  async function handleMerge(branch: Branch) {
    setMergeTarget(branch)
    setConflicts(null)
    setMergeError(null)
    try {
      // apiClient attaches `Authorization: Bearer <token>` from localStorage
      // via the request interceptor (D-22-02). A 409 rejects in axios — the
      // catch block parses { conflicts } for the interactive resolution UI.
      const response = await apiClient.post(`/branches/${encodeURIComponent(branch.name)}/merge`)
      if (response.status === 200) {
        await fetchBranches()
        setMergeTarget(null)
      }
    } catch (err: unknown) {
      const status = axiosStatus(err)
      if (status === 409) {
        const body = axiosData<MergeConflict[] | { conflicts?: MergeConflict[] }>(err)
        const list = Array.isArray(body) ? body : body?.conflicts ?? []
        setConflicts(list)
      } else {
        setMergeError(errorMessage(err, 'Merge failed'))
      }
    }
  }

  function abortMerge() {
    setMergeTarget(null)
    setConflicts(null)
    setMergeError(null)
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Branches</h1>
        <div className="flex gap-2">
          <button
            onClick={() => setDiffOpen(true)}
            className="btn btn-secondary"
            title="Compare commit history for a rule across versions (FRONTEND-01)"
          >
            Compare versions
          </button>
          <button
            onClick={() => setShowCreateModal(true)}
            className="btn btn-primary"
          >
            New Branch
          </button>
        </div>
      </div>

      {/* Branch List */}
      <div className="card">
        {loading ? (
          <p className="text-gray-500">Loading branches…</p>
        ) : error ? (
          <p className="text-red-600">{error}</p>
        ) : branches.length === 0 ? (
          <p className="text-gray-500">No branches found.</p>
        ) : (
          <div className="space-y-2">
            {branches.map((branch) => (
              <div
                key={branch.name}
                className={`flex items-center justify-between p-3 rounded-lg ${
                  branch.isDefault ? 'border-l-4 border-blue-500' : ''
                }`}
              >
                <div>
                  <span className="font-medium">{branch.name}</span>
                  {branch.isDefault && (
                    <span className="badge badge-sm badge-primary ml-2">main</span>
                  )}
                  {branch.lastCommitSha && (
                    <span className="text-xs text-gray-500 ml-2">
                      {branch.lastCommitSha.slice(0, 7)}
                    </span>
                  )}
                  {branch.lastCommitMessage && (
                    <span className="text-xs text-gray-500 ml-2">
                      {branch.lastCommitMessage}
                    </span>
                  )}
                </div>

                {!branch.isDefault && (
                  <button
                    onClick={() => handleMerge(branch)}
                    className="btn btn-sm btn-primary"
                  >
                    Merge to main
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Create Branch Modal */}
      {showCreateModal && (
        <CreateBranchModal
          onClose={() => setShowCreateModal(false)}
          onCreated={() => {
            setShowCreateModal(false)
            fetchBranches()
          }}
        />
      )}

      {/* Merge / Conflict-resolution Modal */}
      {mergeTarget && (
        <MergeModal
          branch={mergeTarget}
          conflicts={conflicts}
          error={mergeError}
          onAbort={abortMerge}
          onResolved={async () => {
            await fetchBranches()
            abortMerge()
          }}
        />
      )}

      {/* Side-by-side diff view (FRONTEND-01 version comparison) */}
      {diffOpen && (
        <DiffModal onClose={() => setDiffOpen(false)} />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Create modal
// ---------------------------------------------------------------------------

function CreateBranchModal({
  onClose,
  onCreated,
}: {
  onClose: () => void
  onCreated: () => void
}) {
  const [name, setName] = useState('')
  const [sourceBranch, setSourceBranch] = useState('main')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function submit() {
    setSubmitting(true)
    setError(null)
    try {
      await apiClient.post('/branches', {
        name: name.trim(),
        source_branch: sourceBranch.trim() || 'main',
      })
      onCreated()
    } catch (err) {
      const status = axiosStatus(err)
      if (status === 409) {
        setError(`Branch "${name}" already exists — choose a different name.`)
      } else if (status === 400) {
        setError(axiosMessage(err) ?? 'Invalid branch name.')
      } else {
        setError(errorMessage(err, 'Failed to create branch'))
      }
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4">
      <div className="card w-full max-w-md">
        <h2 className="text-xl font-bold mb-4">Create New Branch</h2>
        <label className="block text-sm font-medium mb-1">Branch name</label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. feature/my-feature"
          className="input input-bordered w-full mb-3"
          autoFocus
        />
        <label className="block text-sm font-medium mb-1">Source branch</label>
        <input
          type="text"
          value={sourceBranch}
          onChange={(e) => setSourceBranch(e.target.value)}
          placeholder="main"
          className="input input-bordered w-full mb-4"
        />
        {error && <p className="text-red-600 text-sm mb-3">{error}</p>}
        <div className="flex justify-end space-x-2">
          <button onClick={onClose} className="btn btn-secondary" disabled={submitting}>
            Cancel
          </button>
          <button
            onClick={submit}
            disabled={!name.trim() || submitting}
            className="btn btn-primary"
          >
            {submitting ? 'Creating…' : 'Create Branch'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Merge / conflict-resolution modal (GOV-04)
// ---------------------------------------------------------------------------

function MergeModal({
  branch,
  conflicts,
  error,
  onAbort,
  onResolved,
}: {
  branch: Branch
  conflicts: MergeConflict[] | null
  error: string | null
  onAbort: () => void
  onResolved: () => void
}) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4">
      <div className="card w-full max-w-2xl max-h-[80vh] overflow-auto">
        <h2 className="text-xl font-bold mb-2">Merge {branch.name} → main</h2>

        {error && <p className="text-red-600 mb-3">{error}</p>}

        {!conflicts && !error && (
          <p className="text-gray-600 mb-3">Merging…</p>
        )}

        {conflicts && (
          <div className="space-y-4">
            <p className="text-amber-700 dark:text-amber-400">
              Merge conflicts detected ({conflicts.length}). Review the
              conflicting paths below, resolve them in your repository, then
              retry the merge.
            </p>
            <ul className="divide-y divide-gray-200 dark:divide-gray-700">
              {conflicts.map((c, i) => (
                <li key={`${c.path}-${i}`} className="py-2">
                  <div className="font-mono text-sm">{c.path}</div>
                  <div className="mt-1 grid grid-cols-1 sm:grid-cols-3 gap-2 text-xs">
                    <VersionPane label="old" text={c.old_version} />
                    <VersionPane label="base" text={c.base_version} />
                    <VersionPane label="new" text={c.new_version} />
                  </div>
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <button onClick={onAbort} className="btn btn-secondary">Abort</button>
              <button
                onClick={onResolved}
                className="btn btn-primary"
                title="Conflicts are reviewed — close and refresh the branch list"
              >
                Acknowledge &amp; close
              </button>
            </div>
          </div>
        )}

        {!conflicts && error && (
          <div className="flex justify-end mt-4">
            <button onClick={onAbort} className="btn btn-secondary">Close</button>
          </div>
        )}
      </div>
    </div>
  )
}

function VersionPane({ label, text }: { label: string; text?: string | null }) {
  return (
    <div className="border rounded p-2 bg-gray-50 dark:bg-gray-900">
      <div className="text-gray-500 uppercase tracking-wide mb-1">{label}</div>
      <pre className="whitespace-pre-wrap break-words">
        {text ?? '—'}
      </pre>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Side-by-side diff view (FRONTEND-01) using /v1/rules/{id}/git-log
// ---------------------------------------------------------------------------

function DiffModal({ onClose }: { onClose: () => void }) {
  const [ruleId, setRuleId] = useState('')
  const [commits, setCommits] = useState<GitCommitEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [leftSha, setLeftSha] = useState<string | null>(null)
  const [rightSha, setRightSha] = useState<string | null>(null)

  async function loadHistory() {
    if (!ruleId.trim()) return
    setLoading(true)
    setError(null)
    try {
      const response = await apiClient.get<GitLogResponse>(
        `/rules/${encodeURIComponent(ruleId.trim())}/git-log`,
      )
      const list = response.data?.commits ?? []
      setCommits(list)
      // Default-select the two most recent commits for comparison.
      setRightSha(list[0]?.sha ?? null)
      setLeftSha(list[1]?.sha ?? list[0]?.sha ?? null)
    } catch (err) {
      setError(errorMessage(err, 'Failed to load commit history'))
      setCommits([])
    } finally {
      setLoading(false)
    }
  }

  const left = commits.find((c) => c.sha === leftSha)
  const right = commits.find((c) => c.sha === rightSha)

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4">
      <div className="card w-full max-w-4xl max-h-[85vh] overflow-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold">Compare versions</h2>
          <button onClick={onClose} className="btn btn-sm btn-secondary">Close</button>
        </div>

        <div className="flex gap-2 mb-4">
          <input
            type="text"
            value={ruleId}
            onChange={(e) => setRuleId(e.target.value)}
            placeholder="Rule ID (e.g. discount-rule)"
            className="input input-bordered flex-1"
            onKeyDown={(e) => {
              if (e.key === 'Enter') loadHistory()
            }}
          />
          <button onClick={loadHistory} className="btn btn-primary" disabled={loading || !ruleId.trim()}>
            {loading ? 'Loading…' : 'Load history'}
          </button>
        </div>

        {error && <p className="text-red-600 mb-3">{error}</p>}

        {commits.length > 0 && (
          <>
            <p className="text-sm text-gray-500 mb-2">
              {commits.length} commits on {ruleId}. Select two to compare
              side-by-side (FRONTEND-01 version comparison).
            </p>
            <div className="grid grid-cols-2 gap-4">
              <DiffColumn
                title="Version A"
                commits={commits}
                selectedSha={leftSha}
                onSelect={setLeftSha}
                entry={left}
              />
              <DiffColumn
                title="Version B"
                commits={commits}
                selectedSha={rightSha}
                onSelect={setRightSha}
                entry={right}
              />
            </div>
          </>
        )}

        {!loading && !error && commits.length === 0 && (
          <p className="text-gray-500">
              Enter a rule ID and load its commit history to compare two
              versions side-by-side.
          </p>
        )}
      </div>
    </div>
  )
}

function DiffColumn({
  title,
  commits,
  selectedSha,
  onSelect,
  entry,
}: {
  title: string
  commits: GitCommitEntry[]
  selectedSha: string | null
  onSelect: (sha: string) => void
  entry?: GitCommitEntry
}) {
  return (
    <div className="border rounded-lg p-3 bg-white dark:bg-gray-800">
      <div className="flex items-center justify-between mb-2">
        <span className="font-medium">{title}</span>
        <select
          value={selectedSha ?? ''}
          onChange={(e) => onSelect(e.target.value)}
          className="input input-bordered input-sm"
        >
          {commits.map((c) => (
            <option key={c.sha} value={c.sha}>
              {c.sha.slice(0, 7)} — {c.message.slice(0, 40)}
            </option>
          ))}
        </select>
      </div>
      {entry ? (
        <dl className="text-sm space-y-1">
          <div>
            <dt className="text-gray-500 inline">SHA: </dt>
            <dd className="font-mono inline">{entry.sha}</dd>
          </div>
          <div>
            <dt className="text-gray-500 inline">Author: </dt>
            <dd className="inline">{entry.author_name}</dd>
          </div>
          <div>
            <dt className="text-gray-500 inline">Date: </dt>
            <dd className="inline">{new Date(entry.committed_at).toLocaleString()}</dd>
          </div>
          <div>
            <dt className="text-gray-500 inline">Message: </dt>
            <dd className="inline">{entry.message}</dd>
          </div>
        </dl>
      ) : (
        <p className="text-gray-500 text-sm">No commit selected.</p>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Axios error helpers (avoid importing axios's error type across the file)
// ---------------------------------------------------------------------------

type AxiosLike = { response?: { status?: number; data?: unknown } }

function axiosStatus(err: unknown): number | undefined {
  if (err && typeof err === 'object' && 'response' in err) {
    const e = err as AxiosLike
    return e.response?.status
  }
  return undefined
}

function axiosData<T>(err: unknown): T | undefined {
  if (err && typeof err === 'object' && 'response' in err) {
    const e = err as AxiosLike
    return e.response?.data as T | undefined
  }
  return undefined
}

function axiosMessage(err: unknown): string | undefined {
  const data = axiosData<{ error?: string; message?: string }>(err)
  return data?.error ?? data?.message
}

function errorMessage(err: unknown, fallback: string): string {
  return axiosMessage(err) ?? fallback
}