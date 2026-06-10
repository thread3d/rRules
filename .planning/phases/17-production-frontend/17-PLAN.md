# Phase 17: Production Frontend Architecture for rRules BRMS

**Phase Number:** 17  
**Milestone:** v4.0 - Production Platform  
**Date:** 2026-06-09  
**Status:** Plan Created  

---

## Executive Summary

Build the production frontend architecture for Milestone v4.0 of the rRules Business Rules Management System. This phase establishes the foundation for a modern React-based web interface with enterprise-grade features including RBAC, approval workflows, and monitoring dashboards.

**Success Criteria:**
1. Full React 19.2+ application with TypeScript 5.5+ and Vite build tooling
2. TanStack Router-based routing with type-safe navigation
3. CodeMirror 6 editor integration for rule syntax highlighting
4. shadcn/ui component library with lucide-react icons
5. Zustand + TanStack Query state management architecture

---

## Phase Overview

### Goals

| Goal | Description |
|------|-------------|
| **G17-01** | Design production frontend architecture (React, TypeScript, routing) |
| **G17-02** | Implement rule management dashboard with filtering and sorting |
| **G17-03** | Build decision table editor with hit policy support |

### Deliverables

| File/Module | Description |
|-------------|-------------|
| `frontend/package.json` | Dependencies: React 19.2, TypeScript 5.5+, Vite, TanStack Router, Zustand, TanStack Query, CodeMirror 6, Recharts, shadcn/ui dependencies |
| `frontend/vite.config.ts` | Vite configuration with React plugin and path aliases |
| `frontend/tsconfig.json` | TypeScript configuration with bundler mode |
| `frontend/src/main.tsx` | Application entry point with TanStack Router setup |
| `frontend/src/App.tsx` | Root component with routing provider |
| `frontend/src/routes/` | TanStack Router route definitions |
| `frontend/src/components/` | Reusable UI components (navbar, sidebar, layout) |
| `frontend/src/features/rules/` | Rule management feature components |
| `frontend/src/hooks/` | Custom hooks for cross-cutting concerns |

---

## Plan Breakdown

### Plan 17-01: Design Production Frontend Architecture

**Objective:** Establish the foundation with tech stack setup, routing architecture, and project structure.

**Tasks:**

1. **Initialize frontend directory structure**
   ```
   frontend/
   ├── public/                    # Static assets
   ├── src/
   │   ├── api/                   # API clients (client.ts, rules.ts)
   │   ├── components/            # Reusable UI components
   │   │   ├── ui/                # shadcn/ui components
   │   │   └── layout/            # Navbar, Sidebar, Footer
   │   ├── features/              # Feature modules (containers)
   │   │   ├── rules/
   │   │   ├── approvals/
   │   │   └── metrics/
   │   ├── hooks/                 # Custom React hooks
   │   │   ├── use-auth.ts
   │   │   └── use-permission.ts
   │   ├── routes/                # TanStack Router definitions
   │   ├── lib/                   # Utility functions (api, auth, utils)
   │   └── types/                 # TypeScript type definitions (api, auth, common)
   ├── index.html
   └── package.json
   ```

2. **Configure Vite build tooling**
   - Install: `vite`, `@vitejs/plugin-react`
   - Configure `vite.config.ts` with React plugin and path aliases (`@/*` → `src/*`)
   - Enable HMR for development experience

3. **Setup TypeScript configuration**
   - Install: `typescript`, `@types/node`, `@types/react`, `@types/react-dom`
   - Configure `tsconfig.json` with:
     - Target: ES2022
     - Module resolution: "bundler" mode
     - Strict type checking enabled
     - Path aliases for imports

4. **Create auth types and implement TanStack Router foundation**
   - Install: `@tanstack/react-router`, `@tanstack/router-devtools`
   - Create `src/types/auth.ts` with User, Role, and Permission type definitions
   - Create root route with context using `createRootRouteWithContext<{auth: {user: User | null; isAuthenticated: boolean}}>`
   - Set up router devtools for development

**Output:**
- Complete frontend directory structure
- `frontend/package.json` with all dependencies
- `vite.config.ts` and `tsconfig.json` configured
- Basic routing skeleton with root layout including auth context types

**Verification:**
- `npm run dev` starts development server on port 5173
- TypeScript compiles without errors (including new type definitions)
- Router renders basic layout with authentication context

---

### Plan 17-02: Implement Rule Management Dashboard

**Objective:** Build the rule management dashboard with filtering, sorting, and CRUD operations.

**Tasks:**

1. **Create API client layer**
   - `src/api/client.ts`: HTTP client using axios with interceptors for auth tokens
     - Configure baseURL from VITE_API_URL environment variable
     - Request interceptor that adds Bearer token from localStorage
     - Response interceptor for error handling (401 redirects)
   - `src/api/rules.ts`: Rules CRUD operations (`getRules`, `createRule`, `updateRule`, `deleteRule`)
   - TypeScript types for rule API responses

2. **Build rules list view with filtering/sorting**
   - `src/features/rules/List.tsx`: Main list component
   - Filtering by category, owner, status (active/inactive/dead)
   - Sorting by name, last modified, hit count
   - Pagination support for large rule sets

3. **Implement RBAC navigation controls**
   - `src/hooks/use-permission.ts`: Permission hook using useMemo-based role checks (matches Research Section 3.4 pattern)
     - Integrate with TanStack Router route definitions via beforeLoad
     - Use in navigation components for conditional rendering of menu items
     - Implement as a custom hook with proper memoization
   - Navigation items conditionally rendered based on user role
   - Action buttons (edit/delete) hidden for insufficient permissions

4. **Add rule health indicators**
   - Visual indicators for zero-hit rules (dead rules)
   - Conflict warnings for overlapping rules
   - Version status badges (draft/active/pending approval)

**Output:**
- `src/api/rules.ts` with full CRUD API integration
- `src/features/rules/List.tsx` with filtering/sorting
- RBAC-aware navigation in layout components

**Verification:**
- Rules list displays with correct data from API
- Filter dropdowns work and narrow results
- Sorting reorders rules correctly
- Permission-based visibility works (admin sees all, viewer sees read-only)

---

### Plan 17-03: Build Decision Table Editor

**Objective:** Implement a spreadsheet-style decision table editor with hit policy support.

**Tasks:**

1. **Integrate CodeMirror 6 for rule editing**
   - Install: `@uiw/react-codemirror`, `@codemirror/lang-javascript`, `@codemirror/theme-one-dark`
   - Create `src/components/RuleEditor.tsx` wrapper
   - JavaScript/TypeScript syntax highlighting for DRL-like rules
   - Dark theme matching IDE conventions

2. **Build decision table grid component**
   - Install: `@tanstack/react-table` (for表格支持)
   - Create `src/features/rules/DecisionTable.tsx`
   - Rows: condition columns, action columns
   - Hit policy display (Unique, Any, Priority, Collect, etc.)
   - Inline cell editing

3. **Implement hit policy selection**
   - Dropdown for hit policy selection (First, Any, Priority, Collect, RuleOrder, Sum, Min, Max, Count)
   - Visual feedback for current hit policy
   - Policy-specific column behaviors (e.g., Sum shows accumulation)

4. **Add validation and error display**
   - Syntax highlighting errors from CodeMirror
   - Missing rule name warnings
   - Incomplete condition/action cell highlighting

**Output:**
- `src/components/RuleEditor.tsx` with CodeMirror 6 integration
- `src/features/rules/DecisionTable.tsx` with full grid functionality
- Hit policy selector component

**Verification:**
- Rule editor opens with syntax highlighting
- Decision table displays columns for conditions and actions
- Hit policy dropdown allows selection of all DMN policies
- Inline cell editing works without crashes

---

## Dependencies

| Phase | Dependency | Reason |
|-------|------------|--------|
| Phase 17-01 | No dependencies | Foundational architecture |
| Phase 17-02 | 17-01 | Requires routing foundation |
| Phase 17-03 | 17-02 | Needs rule editing infrastructure |

---

## Tech Stack (Phase 17)

### Core Framework
| Package | Version | Purpose |
|---------|---------|---------|
| `react` | 19.2+ | Core UI library with Server Components |
| `react-dom` | 19.2+ | DOM rendering |
| `typescript` | 5.5+ | Type checking with bundler mode |

### Build Tooling
| Package | Version | Purpose |
|---------|---------|---------|
| `vite` | 5.4+ | Dev server and bundler with esbuild |
| `@vitejs/plugin-react` | 4.3+ | React Fast Refresh integration |

### Styling
| Package | Version | Purpose |
|---------|---------|---------|
| `tailwindcss` | 3.4+ | Utility-first CSS framework |
| `postcss` | 8.4+ | CSS processing pipeline |
| `autoprefixer` | 10.4+ | Automatic vendor prefixing |

### Routing & Navigation
| Package | Version | Purpose |
|---------|---------|---------|
| `@tanstack/react-router` | 1.8+ | Type-safe routing with loaders |
| `@tanstack/router-devtools` | 1.8+ | Development debugging panel |

### State Management
| Package | Version | Purpose |
|---------|---------|---------|
| `zustand` | 4.4+ | Client state management (simple API) |
| `immer` | 10.0+ | Immutable updates for complex state |
| `@tanstack/react-query` | 5.50+ | Server state caching and sync |

### Editor Components
| Package | Version | Purpose |
|---------|---------|---------|
| `@uiw/react-codemirror` | 4.23+ | React wrapper for CodeMirror 6 |
| `@codemirror/lang-javascript` | 6.2+ | JS/TS syntax highlighting |
| `@codemirror/theme-one-dark` | 6.1+ | Dark theme for IDE-like editing |

### UI Components & Icons
| Package | Version | Purpose |
|---------|---------|---------|
| `lucide-react` | 0.347+ | Icon library (5KB gzipped) |
| `class-variance-authority` | 0.7+ | Class name utility for shadcn/ui |
| `clsx` | 2.1+ | Class name construction |
| `tailwind-merge` | 2.5+ | Tailwind class merging |

### Testing
| Package | Version | Purpose |
|---------|---------|---------|
| `vitest` | 2.0+ | Unit testing framework |
| `@testing-library/react` | 16.0+ | Component testing utilities |
| `jsdom` | 25.0+ | DOM environment for tests |

---

## File Structure

```
frontend/
├── public/                      # Static assets
│   └── favicon.ico
├── src/
│   ├── api/                     # API clients
│   │   ├── client.ts            # HTTP client with interceptors
│   │   ├── auth.ts              # Auth API (login, logout)
│   │   ├── rules.ts             # Rules CRUD operations
│   │   └── metrics.ts           # Metrics/monitoring API
│   ├── components/              # Reusable UI components
│   │   ├── ui/                  # shadcn/ui primitives
│   │   │   ├── button.tsx
│   │   │   ├── card.tsx
│   │   │   ├── dropdown-menu.tsx
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
│   │       │   └── hit-policies.tsx
│   │       └── metrics-dashboard/
│   │           └── index.tsx
│   ├── features/                # Feature modules (containers)
│   │   ├── rules/
│   │   │   ├── List.tsx
│   │   │   ├── Create.tsx
│   │   │   ├── Edit.tsx
│   │   │   └── Detail.tsx
│   │   ├── approvals/
│   │   │   ├── Pending.tsx
│   │   │   └── History.tsx
│   │   └── metrics/
│   │       ├── Dashboard.tsx
│   │       └── Alerts.tsx
│   ├── hooks/                   # Custom React hooks
│   │   ├── use-auth.ts
│   │   ├── use-permission.ts
│   │   └── use-metrics.ts
│   ├── lib/                     # Utility functions
│   │   ├── api.ts               # API utilities (interceptors)
│   │   ├── auth.ts              # Auth utilities (token handling)
│   │   └── utils.ts             # General helpers
│   ├── routes/                  # TanStack Router definitions
│   │   ├── _layout.tsx          # Layout route with navbar/sidebar
│   │   ├── index.tsx            # Home page
│   │   ├── rules/
│   │   │   ├── index.tsx        # Rules list
│   │   │   ├── $ruleId.tsx      # Dynamic rule detail
│   │   │   └── new.tsx          # New rule creation
│   │   └── __auth/              # Auth-protected routes wrapper
│   │       └── index.tsx
│   ├── types/                   # TypeScript type definitions
│   │   ├── api.ts               # API response types
│   │   ├── auth.ts              # Auth-related types (User, Role)
│   │   └── common.ts            # Common interfaces (Rule, HitPolicy)
│   ├── App.tsx                  # Root component with router provider
│   ├── main.tsx                 # Application entry point
│   └── global.css               # Global styles (Tailwind imports)
├── vite.config.ts
├── tailwind.config.ts
├── postcss.config.js
├── tsconfig.json
└── package.json
```

---

## Requirements Mapping

### Phase 17 Requirements (from ROADMAP.md)

| Plan | Requirement IDs | Description |
|------|-----------------|-------------|
| 17-01 | FRONTEND-01 | Web-based IDE with syntax highlighting for Drools/DRL rules |
| 17-02 | FRONTEND-02, FRONTEND-01 | Rule management dashboard with filtering and sorting |
| 17-03 | FRONTEND-01, GOV-01 | Decision table editor with hit policy support |

### Success Criteria Coverage

At completion of Phase 17 plans, the following must be verifiable:

1. **Frontend foundation exists** - React 19.2+ application with Vite, TypeScript 5.5+, TanStack Router
2. **Routing works** - Type-safe navigation with route definitions for all features
3. **API integration ready** - HTTP client with interceptors for auth tokens
4. **Rule editor functional** - CodeMirror 6 integrated with syntax highlighting
5. **Dashboard operational** - Rules list with filtering, sorting, and RBAC controls

---

## Verification Plan

### Unit Tests Per Feature

```bash
# Run all tests
npm run test

# Run specific feature tests
npm run test -- features/rules
```

### Integration Checks

1. **Development Server**: `npm run dev` → visit http://localhost:5173
2. **Build Production**: `npm run build` → no errors, output in `dist/`
3. **Type Checking**: `npx tsc --noEmit` → zero errors

### Manual Verification Checklist

- [ ] Development server starts without errors
- [ ] Routing navigates between pages correctly
- [ ] Rules list displays from mock API (or real backend when available)
- [ ] Filtering and sorting work on rules list
- [ ] Permission-based UI elements show/hide correctly
- [ ] Rule editor opens with syntax highlighting
- [ ] Decision table grid renders columns for conditions/actions

---

## Next Steps

1. Execute Phase 17 plans in order: 17-01 → 17-02 → 17-03
2. Each plan produces deployable increments of the frontend application
3. After all plans complete, integrate with Rust backend (Phase 20)
4. Proceed to Milestone v4.0 governance and monitoring features

---

## Notes

- **CodeMirror 6 vs Monaco**: CodeMirror selected for bundle size (~50-200kB vs ~5MB) - sufficient for rule syntax highlighting
- **TanStack Router over React Router**: Superior type safety and built-in data loading patterns
- **shadcn/ui default**: Component source code included (no black box), WCAG compliant, Tailwind CSS
- **Multi-stage Docker build**: Production images ~25-40MB vs 1.5GB development images

---

## Environment Variables

```bash
# .env.development
VITE_API_URL=http://localhost:8080/api/v1

# .env.production
VITE_API_URL=https://api.rrules.example.com/api/v1
```

---

*Phase plan created: 2026-06-09*
*Based on research at .planning/phases/17-production-frontend/17-RESEARCH.md*
