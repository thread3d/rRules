import { createFileRoute } from '@tanstack/react-router'
import { Dashboard } from '../monitoring/Dashboard'

export const Route = createFileRoute('/monitoring')({
  component: Dashboard,
})
