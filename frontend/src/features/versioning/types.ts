// Git versioning types for Phase 18 Enterprise Governance

export interface Branch {
  id: number
  name: string
  isDefault: boolean
  createdAt: string
  lastCommitSha?: string
  lastCommitMessage?: string
}

export type DiffStatus = 'added' | 'modified' | 'deleted' | 'renamed' | 'typeChanged' | 'untracked'

export interface DiffEntry {
  oldPath?: string
  newPath?: string
  status: DiffStatus
  oldContent?: string
  newContent?: string
}

export interface MergeResult {
  success: boolean
  commitSha?: string
  conflicts: Conflict[]
}

export interface Conflict {
  path: string
  oldVersion?: string
  baseVersion?: string
  newVersion?: string
}

export interface CommitInfo {
  sha: string
  message: string
  author: string
  timestamp: string
}
