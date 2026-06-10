# Phase 17 Research: Production Frontend Architecture for rRules BRMS

**Phase:** 17 - Production Frontend Architecture  
**Date:** 2026-06-09  
**Status:** Research Complete

---

## Executive Summary

This research establishes the production frontend architecture for Milestone v4.0 of the rRules Business Rules Management System. The goal is to build a React-based rule authoring interface with enterprise-grade features including RBAC, audit trails, and monitoring dashboards.

Key findings for 2026:

1. **React/TypeScript Stack**: React 19.2 with TypeScript 5.5+ is the production baseline. TanStack Router v1.8+ has emerged as the preferred routing solution over React Router due to superior type safety and built-in data loading patterns.

2. **State Management**: A hybrid approach is recommended - TanStack Query for server state (caching, refetching) combined with Zustand for client state (simpler API, smaller bundle). Jotai is viable if atomic state updates are critical.

3. **IDE Components**: For the rRules use case (rule syntax highlighting without full IDE features), CodeMirror 6 is preferred over Monaco Editor due to significantly smaller bundle size (~50-200kB vs ~5MB) while still providing excellent syntax highlighting and accessibility.

4. **Production Deployment**: Multi-stage Docker builds reduce image size from ~1.5GB to ~25-40MB. Key optimizations include using nginx:alpine, layer caching with `npm ci`, and BuildKit cache mounts.

**Primary Recommendation:** Build Phase 17's frontend on a Vite + React 19 + TypeScript 5.5 foundation, using TanStack Router for routing, Zustand + TanStack Query for state management, CodeMirror 6 for the rule editor, and Recharts for dashboards.

---

## 1. Modern React/TypeScript Architecture

### 1.1 Core Framework

| Technology | Recommended Version | Rationale |
|------------|---------------------|-----------|
| **React** | 19.2+ (December 2024) | Production-ready with Server Components, Actions, and Compiler integration |
| **TypeScript** | 5.5+ | Module resolution "bundler" mode for Vite/esbuild support |

### 1.2 Build Tooling

**Vite vs Create React App (CRA):**
- **Vite**: Required for modern development experience. Uses esbuild for fast HMR, native ES imports, and better TypeScript integration.
- **CRA**: Deprecated; no longer maintained with modern toolchains.

**Recommended Vite Configuration:**
```ts
// vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': '/src',
    },
  },
})
```

### 1.3 TypeScript Configuration

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["DOM", "DOM.Iterable", "ES2022"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",

    "strict": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "noImplicitReturns": true,

    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "skipLibCheck": true,
    
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist"]
}
```

### 1.4 React 19 Key Features

| Feature | Production Impact |
|---------|-------------------|
| `useActionState` + `<form action={}>` | Reduces form boilerplate by ~70% |
| Server Components | Zero JS shipped for display-only content |
| React Compiler v1+ | Automatic memoization, eliminates `useMemo`/`useCallback` bugs |

---

## 2. IDE-Style Rule Editing Components

### 2.1 Editor Comparison (CodeMirror 6 vs Monaco)

| Factor | Monaco Editor | CodeMirror 6 | Verdict for rRules |
|--------|---------------|--------------|-------------------|
| Bundle size | ~5MB gzipped | ~50-200kB | **CodeMirror wins** |
| Features | Full VS Code IntelliSense, LSP | Syntax highlighting only (LSP via plugin) | Monaco if full IDE needed |
| Mobile support | Limited | Excellent | CodeMirror wins |
| Accessibility | Basic | Excellent | CodeMirror wins |
| React integration | `@monaco-editor/react` | `@uiw/react-codemirror` or vanilla | Both viable |

**Recommendation for rRules:**
- Use **CodeMirror 6** as primary editor - the rule syntax (DRL-like) doesn't require full LSP IntelliSense, and bundle size savings (~4.8MB) are significant
- Add [LSP support via `codemirror-languageserver`](https://github.com/codemirror/langauge-server) only if real-time autocomplete becomes critical

### 2.2 CodeMirror 6 Integration Pattern

```tsx
// editor/RuleEditor.tsx
import { useState, useEffect } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { javascript } from '@codemirror/lang-javascript'
import { python } from '@codemirror/lang-python'
import { EditorState, Compartment, EditorStateConfig } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'

interface RuleEditorProps {
  value: string
  onChange: (value: string) => void
}

export function RuleEditor({ value, onChange }: RuleEditorProps) {
  const [cmValue, setCmValue] = useState(value)
  
  useEffect(() => {
    if (value !== cmValue) {
      setCmValue(value)
    }
  }, [value])

  return (
    <CodeMirror
      value={cmValue}
      height="500px"
      theme={oneDark}
      extensions={[
        javascript({ typescript: true }),
        EditorState.readOnly.of(false),
      ]}
      onChange={(val) => {
        setCmValue(val)
        onChange(val)
      }}
    />
  )
}
```

### 2.3 Supporting Libraries

| Library | Purpose |
|---------|---------|
| `@uiw/react-codemirror` | React wrapper for CodeMirror 6 |
| `@codemirror/lang-javascript` | JavaScript/TypeScript syntax highlighting |
| `@codemirror/lang-python` | Python highlighting (for FEEL expressions) |
| `@codemirror/theme-one-dark` | Dark theme matching IDE conventions |
| `@codemirror/language-data` | Language definition extensions |

---

## 3. Enterprise UI Patterns

### 3.1 Component Libraries

**shadcn/ui** is the recommended base for enterprise React applications in 2026:

- Built on Radix UI primitives
- Fully accessible (WCAG compliant)
- Tailwind CSS styling with `@tailwind` directives
- Component source code included (no black box)
- Actively maintained with frequent updates

**Installation:**
```bash
npm install -D tailwindcss postcss autoprefixer
npx shadcn-ui@latest init
```

### 3.2 Icon Libraries

| Library | Bundle Size (50 icons) | Tree-shaking | Recommendation |
|---------|------------------------|--------------|----------------|
| **lucide-react** | ~5 KB gzipped | Full ESM support | **Recommended for rRules** |
| react-icons | ~81 KB gzipped | CJS only | Avoid for new projects |

**Recommendation:** Use `lucide-react` for all UI icons in Phase 17. It's the default with shadcn/ui and provides consistent design.

### 3.3 RBAC UI Patterns

**Role-based Navigation:**
```tsx
// components/Navbar.tsx
import { useAuth } from '@/hooks/use-auth'
import { Link, useLocation } from '@tanstack/react-router'

const navItems = [
  { label: 'Rules', path: '/rules', requiredRole: 'viewer' },
  { label: 'New Rule', path: '/rules/new', requiredRole: 'author' },
  { label: 'Approvals', path: '/approvals', requiredRole: 'reviewer' },
  { label: 'Settings', path: '/settings', requiredRole: 'admin' },
]

export function Navbar() {
  const { user } = useAuth()
  
  return (
    <nav className="flex space-x-4">
      {navItems.map(item => {
        if (!user || !hasRole(user.role, item.requiredRole)) return null
        return (
          <Link key={item.path} to={item.path}>
            {item.label}
          </Link>
        )
      })}
    </nav>
  )
}
```

**Permission-Hook Pattern:**
```tsx
// hooks/use-permission.ts
import { useMemo } from 'react'
import { useAuth } from './use-auth'

type Permission = 'read' | 'write' | 'delete' | 'approve'

export function usePermission(permission: Permission, resource?: string) {
  const { user, rolePermissions } = useAuth()
  
  return useMemo(() => {
    if (!user || !resource) return false
    const permissions = rolePermissions[user.role] || []
    return permissions.includes(permission)
  }, [user, permission, resource])
}
```

### 3.4 Audit Trail Visualization

```tsx
// components/AuditTrail.tsx
import { format } from 'date-fns'

interface AuditEvent {
  id: string
  action: string
  actor: string
  timestamp: string
  details?: Record<string, unknown>
}

export function AuditTrail({ events }: { events: AuditEvent[] }) {
  return (
    <div className="space-y-4">
      {events.map(event => (
        <div key={event.id} className="flex space-x-4 border-l-2 pl-4">
          <div className="w-32 text-sm text-gray-500">
            {format(new Date(event.timestamp), 'MMM d, HH:mm')}
          </div>
          <div>
            <div className="font-medium">{event.actor}</div>
            <div className="text-sm text-gray-600">{event.action}</div>
            {event.details && (
              <pre className="mt-2 text-xs font-mono bg-gray-100 p-2 rounded">
                {JSON.stringify(event.details, null, 2)}
              </pre>
            )}
          </div>
        </div>
      ))}
    </div>
  )
}
```

### 3.5 Approval Workflow UI

```tsx
// components/ApprovalWorkflow.tsx
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader } from '@/components/ui/card'

interface ApprovalRequest {
  id: string
  title: string
  submittedBy: string
  status: 'pending' | 'approved' | 'rejected'
}

export function ApprovalWorkflow({ requests }: { requests: ApprovalRequest[] }) {
  const [approving, setApproving] = useState<string | null>(null)
  
  const handleApprove = async (id: string) => {
    setApproving(id)
    // Call API to approve
    // ...
    setApproving(null)
  }
  
  return (
    <div className="space-y-4">
      {requests.map(request => (
        <Card key={request.id}>
          <CardHeader>
            <div className="flex justify-between items-center">
              <span>{request.title}</span>
              <span className={`text-sm ${
                request.status === 'pending' ? 'text-yellow-600' :
                request.status === 'approved' ? 'text-green-600' :
                'text-red-600'
              }`}>
                {request.status}
              </span>
            </div>
          </CardHeader>
          <CardContent className="flex justify-end space-x-2">
            {request.status === 'pending' && (
              <>
                <Button
                  variant="outline"
                  onClick={() => handleApprove(request.id)}
                  disabled={approving === request.id}
                >
                  Approve
                </Button>
                <Button variant="destructive">Reject</Button>
              </>
            )}
          </CardContent>
        </Card>
      ))}
    </div>
  )
}
```

---

## 4. Monitoring Dashboard Design

### 4.1 Chart Library Selection (2026)

| Library | GitHub Stars | Weekly Downloads | Best For |
|---------|-------------|------------------|----------|
| **Recharts** | 27k+ | 48M+ | shadcn/ui projects, SSR-friendly |
| **react-chartjs-2** | 6.8k+ | 3.7M+ | Canvas-based, real-time updates |
| **Nivo** | 14k+ | 760k+ | Polished dashboards with animations |
| **Apache ECharts** | 66k+ | 2.7M+ | Large datasets, exotic chart types |

**Recommendation:** Use **Recharts** for rRules dashboards - it's the default in shadcn/ui ecosystem and excellent for metrics visualization.

### 4.2 Prometheus Metric Visualization

```tsx
// components/MetricsDashboard.tsx
import { 
  LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid,
  Tooltip, Legend, ResponsiveContainer 
} from 'recharts'

interface MetricsData {
  timestamp: string
  rulesFired: number
  executionTimeMs: number
  memoryUsageMB: number
}

export function MetricsDashboard({ metrics }: { metrics: MetricsData[] }) {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
      {/* Rules Fired Over Time */}
      <ResponsiveContainer height={300}>
        <LineChart data={metrics}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="timestamp" />
          <YAxis />
          <Tooltip />
          <Legend />
          <Line 
            type="monotone" 
            dataKey="rulesFired" 
            name="Rules Fired"
            stroke="#8884d8" 
            strokeWidth={2}
          />
        </LineChart>
      </ResponsiveContainer>

      {/* Execution Time Distribution */}
      <ResponsiveContainer height={300}>
        <BarChart data={metrics}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="timestamp" />
          <YAxis unit="ms" />
          <Tooltip />
          <Legend />
          <Bar 
            dataKey="executionTimeMs" 
            name="Execution Time (ms)"
            fill="#82ca9d"
          />
        </BarChart>
      </ResponsiveContainer>

      {/* Memory Usage */}
      <ResponsiveContainer height={300} className="col-span-1 lg:col-span-2">
        <LineChart data={metrics}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="timestamp" />
          <YAxis unit="MB" />
          <Tooltip />
          <Legend />
          <Line 
            type="monotone" 
            dataKey="memoryUsageMB" 
            name="Memory Usage"
            stroke="#ffc658" 
            strokeWidth={2}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}
```

### 4.3 Real-Time Updates with WebSockets

```tsx
// hooks/use-metrics.ts
import { useEffect, useState } from 'react'

interface Metrics {
  timestamp: string
  rulesFired: number
  executionTimeMs: number
}

export function useMetrics() {
  const [metrics, setMetrics] = useState<Metrics[]>([])
  
  useEffect(() => {
    // Initial fetch
    fetch('/api/metrics?limit=100')
      .then(r => r.json())
      .then(data => setMetrics(data))
    
    // WebSocket for real-time updates
    const ws = new WebSocket('ws://localhost:8080/api/metrics/stream')
    
    ws.onmessage = (event) => {
      const metric = JSON.parse(event.data)
      setMetrics(prev => [...prev, metric].slice(-100))
    }
    
    return () => ws.close()
  }, [])
  
  return { metrics }
}
```

---

## 5. Production Deployment Considerations

### 5.1 Multi-Stage Docker Build

```dockerfile
# Stage 1: Build with Node.js
FROM node:20-alpine AS build
WORKDIR /app

# Copy package files first for caching
COPY package*.json ./

# Install dependencies (cached unless package.json changes)
RUN npm ci --only=production

# Copy source code
COPY . .

# Build the application
RUN npm run build

# Stage 2: Production with nginx
FROM nginx:alpine AS production

# Copy built assets
COPY --from=build /app/dist /usr/share/nginx/html

# Custom nginx configuration for SPA routing and caching
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
```

**nginx.conf:**
```nginx
server {
    listen 80;
    server_name _;
    root /usr/share/nginx/html;
    index index.html;

    # SPA routing - redirect all requests to index.html
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Static assets caching
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # Health check endpoint
    location /health {
        access_log off;
        return 200 'ok';
        add_header Content-Type text/plain;
    }
}
```

### 5.2 Build Optimizations

```ts
// vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  
  // Code splitting configuration
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            // Split vendor libraries into separate chunks
            if (id.includes('react') || id.includes('react-dom')) {
              return 'react-vendor'
            }
            if (id.includes('@tanstack')) {
              return 'tanstack-vendor'
            }
            return 'vendor'
          }
        },
      },
    },
    chunkSizeWarningLimit: 600,
  },
  
  // Optimize dependencies
  optimizeDeps: {
    include: [
      '@tanstack/react-query',
      '@tanstack/react-router',
      'zustand',
      'lucide-react'
    ],
  },
})
```

### 5.3 Environment Variables

```bash
# .env.production
VITE_API_URL=https://api.rrules.example.com
VITE_WS_URL=wss://api.rrules.example.com/metrics
VITE_PROMETHEUS_URL=http://prometheus:9090
VITE_APP_NAME="rRules"
```

### 5.4 Kubernetes Deployment

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rrules-frontend
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rrules-frontend
  template:
    metadata:
      labels:
        app: rrules-frontend
    spec:
      containers:
      - name: frontend
        image: rrules/frontend:v4.0.0
        ports:
        - containerPort: 80
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "200m"
        livenessProbe:
          httpGet:
            path: /health
            port: 80
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 80
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: rrules-frontend
spec:
  selector:
    app: rrules-frontend
  ports:
  - port: 80
    targetPort: 80
  type: ClusterIP
```

---

## 6. Recommended Tech Stack for Phase 17

### 6.1 Core Framework

| Package | Version | Purpose |
|---------|---------|---------|
| `react` | 19.2+ | Core UI library |
| `react-dom` | 19.2+ | DOM rendering |
| `typescript` | 5.5+ | Type checking |

### 6.2 Build Tooling

| Package | Version | Purpose |
|---------|---------|---------|
| `vite` | 5.4+ | Dev server and bundler |
| `@vitejs/plugin-react` | 4.3+ | React Fast Refresh |
| `tailwindcss` | 3.4+ | Styling framework |
| `postcss` | 8.4+ | CSS processing |

### 6.3 Routing & Navigation

| Package | Version | Purpose |
|---------|---------|---------|
| `@tanstack/react-router` | 1.8+ | Type-safe routing |
| `@tanstack/router-devtools` | 1.8+ | Dev tools panel |

### 6.4 State Management

| Package | Version | Purpose |
|---------|---------|---------|
| `@tanstack/react-query` | 5.50+ | Server state & caching |
| `zustand` | 4.4+ | Client state management |
| `immer` | 10.0+ | Immutable updates |

### 6.5 Editor Components

| Package | Version | Purpose |
|---------|---------|---------|
| `@uiw/react-codemirror` | 4.23+ | React wrapper for CodeMirror |
| `@codemirror/lang-javascript` | 6.2+ | JS/TS highlighting |
| `@codemirror/theme-one-dark` | 6.1+ | Dark theme |

### 6.6 UI Components

| Package | Version | Purpose |
|---------|---------|---------|
| `lucide-react` | 0.347+ | Icon library |
| `class-variance-authority` | 0.7+ | Class name utility |
| `clsx` | 2.1+ | Class name construction |
| `tailwind-merge` | 2.5+ | Tailwind class merging |

### 6.7 Testing

| Package | Version | Purpose |
|---------|---------|---------|
| `vitest` | 2.0+ | Unit testing framework |
| `@testing-library/react` | 16.0+ | Component testing |
| `@vitejs/plugin-react-swc` | 3.7+ | Faster React transforms |

### 6.8 Package Installation

```bash
# Install core dependencies
npm install react react-dom
npm install typescript @types/node @types/react @types/react-dom

# Install build tools
npm install -D vite @vitejs/plugin-react tailwindcss postcss autoprefixer

# Install routing and state management
npm install @tanstack/react-router @tanstack/router-devtools
npm install zustand immer

# Install editor components
npm install @uiw/react-codemirror
npm install @codemirror/lang-javascript @codemirror/theme-one-dark

# Install UI components
npm install lucide-react class-variance-authority clsx tailwind-merge

# Install testing dependencies
npm install -D vitest @testing-library/react jsdom
```

---

## 7. File Structure Recommendations

### 7.1 Recommended Project Structure

```
frontend/
├── public/                      # Static assets
│   └── favicon.ico
│   └── manifest.json
├── src/
│   ├── api/                     # API clients
│   │   ├── client.ts            # HTTP client (axios/fetch)
│   │   ├── auth.ts              # Authentication API
│   │   ├── rules.ts             # Rules API
│   │   └── metrics.ts           # Metrics/monitoring API
│   ├── components/              # Reusable UI components
│   │   ├── ui/                  # shadcn/ui components
│   │   │   ├── button.tsx
│   │   │   ├── card.tsx
│   │   │   └── ...
│   │   ├── layout/              # Layout components
│   │   │   ├── navbar.tsx
│   │   │   ├── sidebar.tsx
│   │   │   └── footer.tsx
│   │   └── features/            # Feature-specific components
│   │       ├── rule-editor/
│   │       │   ├── index.tsx
│   │       │   └── editor-utils.ts
│   │       ├── decision-table/
│   │       │   ├── index.tsx
│   │       │   └── cell-editors.tsx
│   │       ├── audit-trail/
│   │       │   └── index.tsx
│   │       └── metrics-dashboard/
│   │           └── index.tsx
│   ├── features/                # Feature modules (containers)
│   │   ├── auth/
│   │   │   ├── login.tsx
│   │   │   ├── register.tsx
│   │   │   └── hooks.ts
│   │   ├── rules/
│   │   │   ├── list.tsx
│   │   │   ├── create.tsx
│   │   │   ├── edit.tsx
│   │   │   └── detail.tsx
│   │   ├── approvals/
│   │   │   ├── pending.tsx
│   │   │   └── history.tsx
│   │   └── metrics/
│   │       ├── dashboard.tsx
│   │       └── alerts.tsx
│   ├── hooks/                   # Custom React hooks
│   │   ├── use-auth.ts
│   │   ├── use-permission.ts
│   │   └── use-metrics.ts
│   ├── lib/                     # Utility functions
│   │   ├── api.ts               # API utilities
│   │   ├── auth.ts              # Auth utilities
│   │   └── utils.ts             # General helpers
│   ├── routes/                  # TanStack Router definition
│   │   ├── _layout.tsx          # Layout route
│   │   ├── index.tsx            # Home page
│   │   ├── rules/
│   │   │   ├── index.tsx        # Rules list
│   │   │   └── $ruleId.tsx      # Dynamic rule route
│   │   ├── __auth/
│   │   │   ├── login.tsx        # Protected routes wrapper
│   │   │   └── register.tsx
│   │   └── __unauthenticated/   # Unauthenticated routes
│   │       ├── index.tsx
│   │       ├── privacy-policy.tsx
│   │       └── terms-of-service.tsx
│   ├── types/                   # TypeScript type definitions
│   │   ├── api.ts               # API response types
│   │   ├── auth.ts              # Auth-related types
│   │   └── common.ts            # Common interfaces
│   ├── App.tsx
│   ├── main.tsx                 # Entry point
│   └── global.css               # Global styles (Tailwind)
├── vite.config.ts
├── tailwind.config.ts
├── postcss.config.js
├── tsconfig.json
└── package.json
```

### 7.2 Route Structure with TanStack Router

```tsx
// src/routes/router.tsx
import { createRootRouteWithContext,Outlet } from '@tanstack/react-router'
import { Layout } from '@/components/layout'

export const Root = createRootRouteWithContext<{
  auth: {
    user: User | null
    isAuthenticated: boolean
  }
}>()({
  component: RootComponent,
})

function RootComponent() {
  const { auth } = useMatch()
  
  if (!auth.isAuthenticated) {
    return <Navigate to="/login" />
  }
  
  return (
    <Layout>
      <Outlet />
    </Layout>
  )
}
```

```tsx
// src/routes/rules/_layout.tsx
import { createRouteConfig, Outlet } from '@tanstack/react-router'
import { hasPermission } from '@/lib/auth'

export const rulesRoute = createRouteConfig({
  path: '/rules',
  component: RulesLayout,
  beforeLoad: ({ context }) => {
    if (!hasPermission(context.auth.user, 'read', 'rule')) {
      throw new Error('Unauthorized')
    }
  },
})()

function RulesLayout() {
  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold">Rules</h1>
        <Button>Create New Rule</Button>
      </div>
      <Outlet />
    </div>
  )
}
```

---

## Sources

### Primary (HIGH confidence)
- [React 19 Release Notes](https://react.dev/blog/2024/12/05/react-19) - React 19.0 release with Actions, Server Components
- [TanStack Router v1.8 Documentation](https://tanstack.com/router/latest/docs/framework/react/overview) - Type-safe routing with loaders and caching
- [TypeScript 5.5 Release Notes](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-5.html) - Module resolution "bundler" mode

### Secondary (MEDIUM confidence)
- [CodeMirror vs Monaco Comparison - PkgPulse](https://www.pkgpulse.com/guides/monaco-editor-vs-codemirror-6-vs-sandpack-in-browser-2026) - Bundle size and feature comparison
- [Zustand vs Redux Toolkit vs Jotai - Better Stack](https://betterstack.com/community/guides/scaling-nodejs/zustand-vs-redux-toolkit-vs-jotai/) - State management comparison
- [Recharts for React Dashboards](https://recharts.github.io/en-US/) - Chart library for metrics visualization

### Tertiary (LOW confidence)
- [React Icons vs Lucide - Icon Search](https://iconsearch.info/blog/react-icons-vs-lucide-react-2026) - Icon library comparison
- [Docker React Multi-Stage Builds - Docker Blog](https://www.docker.com/blog/how-to-dockerize-react-app/) - Containerization patterns

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Vite is the appropriate build tool for Phase 17 frontend | Sections 1.3, 5 | Low - Vite has become the de facto standard for new React projects in 2026 |
| A2 | CodeMirror 6 bundle size (~50-200kB) is acceptable vs Monaco (~5MB) | Section 2.1 | Medium - If full IDE features become critical later, migration would be needed |
| A3 | Zustand provides adequate state management for client state | Sections 4, 6 | Low - Zustand's simplicity aligns with rRules' expected complexity; can migrate to Jotai/RTK if needed |
| A4 | shadcn/ui is the appropriate UI component library | Section 3.1 | Medium - Requires Tailwind dependency; alternative would be MUI or Ant Design |

---

## Open Questions

1. **API Integration**
   - What authentication mechanism does the Rust API use (JWT, session cookies)?
   - Are there existing API documentation/OpenAPI specs to guide frontend development?

2. **Rule Editor Requirements**
   - Does rRules need full LSP IntelliSense support for rule writing?
   - Should the editor support multiple rule syntaxes (DRL, FEEL, custom)?

3. **Data Source**
   - Will the dashboard pull metrics from Prometheus directly or via the Rust API layer?

4. **Mobile Support**
   - Is mobile/tablet support required for Phase 17, or can it be deferred to a future phase?

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Node.js | Build/Dev server | Check needed | v20+ recommended | N/A |
| npm/yarn/pnpm | Package management | Check needed | Any modern version | pnpm preferred for mono-repo |
| Docker | Production deployment | Check needed | v24+ | Kubernetes manifests provided |

**Missing dependencies with no fallback:**
- Node.js v20+ (build tooling requires this minimum)

**Missing dependencies with fallback:**
- None identified

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Vitest 2.0+ |
| Config file | `vitest.config.ts` |
| Quick run command | `npx vitest` |
| Full suite command | `npx vitest run` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FRONTEND-01 | Web-based IDE with syntax highlighting | integration | `vitest src/features/editor` | ❌ Wave 0 |
| FRONTEND-02 | Rule management dashboard filtering/sorting | unit | `vitest src/features/rules` | ❌ Wave 0 |
| GOV-01 | RBAC role-based navigation | integration | `vitest src/hooks/use-permission` | ❌ Wave 0 |
| GOV-02 | Approval workflow UI | integration | `vitest src/features/approvals` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `npx vitest`
- **Per wave merge:** `npx vitest run`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/features/rules/list.test.tsx` — covers FRONTEND-02
- [ ] `src/features/approvals/pending.test.tsx` — covers GOV-02
- [ ] `vitest.config.ts` — framework configuration
- [ ] `src/test-utils.tsx` — shared test utilities

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | JWT tokens via `@tanstack/react-query` interceptors |
| V3 Session Management | yes | HttpOnly cookies managed by Rust backend |
| V4 Access Control | yes | RBAC hooks in frontend routing and UI components |
| V5 Input Validation | yes | TypeScript types + Zod validation schemas |
| V6 Cryptography | yes | `window.crypto.subtle` for client-side hashing |

### Known Threat Patterns for React Frontend

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| XSS via rule content | Spoofing | Content-Security-Policy header + escape user input |
| CSRF attacks | Tampering | SameSite cookies + anti-CSRF tokens |
| Sensitive data in state | Elevation of Privilege | Clear state on logout, avoid localStorage for secrets |
| Insecure direct object references | Information Disclosure | Backend authorization checks + client-side permission hooks |

---

**Research date:** 2026-06-09  
**Valid until:** 2026-07-09 (30 days for stable tech stack, 7 days for fast-moving frameworks)
