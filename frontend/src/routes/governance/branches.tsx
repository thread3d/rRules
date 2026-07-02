import { createFileRoute } from '@tanstack/react-router'
import { BranchManager } from '@/features/versioning/BranchManager'

// TODO(22-06): wire BranchManager to the branching API (mount-only here).
export const Route = createFileRoute('/governance/branches')({
  component: BranchManager,
})