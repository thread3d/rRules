// The manager fetches the branching API itself (`features/versioning/BranchManager`):
// list, create, merge and diff all call `/v1/branches`. This route only mounts it.
import { createFileRoute } from '@tanstack/react-router'
import { BranchManager } from '@/features/versioning/BranchManager'

export const Route = createFileRoute('/governance/branches')({
  component: BranchManager,
})