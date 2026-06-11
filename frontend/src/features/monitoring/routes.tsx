// Monitoring routes for rRules BRMS

import { createBrowserRouter } from 'react-router-dom'
import {
  EngineMetricsDashboard,
  RuleCoverageDashboard,
  DeadRuleReportDashboard,
} from './dashboards'

export const monitoringRoutes = createBrowserRouter([
  {
    path: '/monitoring',
    children: [
      {
        index: true,
        element: <EngineMetricsDashboard />,
      },
      {
        path: 'coverage',
        element: <RuleCoverageDashboard />,
      },
      {
        path: 'dead-rules',
        element: <DeadRuleReportDashboard />,
      },
    ],
  },
])
