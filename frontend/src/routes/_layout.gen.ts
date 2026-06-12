// Auto-generated TanStack Router configuration
import type { Route } from '@tanstack/react-router'
import { IndexComponent } from './index'
import { RulesPage } from './rules'

export const routeTree = {
  path: '/',
  children: [
    {
      id: 'index',
      path: '/',
      component: IndexComponent,
    },
    {
      id: 'rules',
      path: '/rules',
      component: RulesPage,
    },
  ],
} as Route
