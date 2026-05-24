# Phase 3: REST API Layer - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-24
**Phase:** 3-REST API Layer
**Areas discussed:** API design approach, Rule execution endpoint, CRUD operations, Docker/K8s deployment

---

## API Design Approach

| Option | Description | Selected |
|--------|-------------|----------|
| REST only | Pure RESTful endpoints; simpler for CRUD operations | |
| GraphQL only | Single endpoint with flexible queries; better for complex data fetching | |
| Hybrid approach | REST for CRUD, GraphQL for rule execution and exploration | ✓ |

**User's choice:** Hybrid approach - REST for CRUD, GraphQL for rule execution

---

## API Versioning Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| URL path versioning | /v1/rules, /v2/rules - most common and straightforward | ✓ |
| Header-based versioning | Accept-Version header; cleaner URLs but less visible | |
| Query param versioning | /rules?version=1; easiest to change per-request | |

**User's choice:** URL path versioning

---

## API Authentication Method

| Option | Description | Selected |
|--------|-------------|----------|
| API keys | Simple key-based auth in header or query param | |
| JWT tokens | Stateless token auth with refresh support | |
| Session-based | Cookie-based sessions tied to user login | ✓ |

**User's choice:** Session-based authentication (requires Phase 4 RBAC integration)

---

## Rule Execution Payload Format

| Option | Description | Selected |
|--------|-------------|----------|
| Simple facts object | "{\"facts\": {...}, \"rules\": [...]}\" - straightforward for basic use | |
| Session-based execution | "{\"session_id\": \"...\", \"actions\": [...]}\" - stateful with existing working memory | |
| Flexible data model | Support both facts object and session_id; auto-detects mode | ✓ |

**User's choice:** Flexible data model - auto-detects between facts and session

---

## Rule Execution Response Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Fired rules + facts | "{\"fired_rules\": [...], \"facts\": {...}, \"metadata\": {}}\" | |
| Execution results object | "{\"results\": {\"success\": true, \"fired\": 5, \"facts_modified\": 3}}" | |
| Full trace output | "\"{execution_id, fired_rules, facts_after, inference_trace}\"" | ✓ |

**User's choice:** Full trace output - includes execution_id, fired_rules, facts_after, inference_trace

---

## Async Rule Execution Support

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, with polling | POST returns job ID; client polls /jobs/{id} for status | |
| No, sync only | All execution is synchronous; timeouts enforced on slow rules | |
| Webhook callbacks | Async jobs post results to callback URL when complete | ✓ |

**User's choice:** Webhook callbacks - async results posted to callback URL

---

## CRUD HTTP Methods

| Option | Description | Selected |
|--------|-------------|----------|
| Standard REST methods | POST /rules (create), GET /rules (list), PUT /rules/{id} (update), DELETE /rules/{id} | ✓ |
| Patch-style updates | PATCH for partial updates; PUT requires full resource | |
| Simplified API | Only POST and GET - all updates are re-creations | |

**User's choice:** Standard REST methods

---

## Bulk Operations Support

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, batch endpoints | POST /rules/bulk for creating multiple rules at once | |
| No, individual only | Each rule must be created/updated/deleted separately | |
| Conditional bulk | Bulk support with transaction option - all-or-nothing | ✓ |

**User's choice:** Conditional bulk - with transaction option

---

## Rules List Filtering

| Option | Description | Selected |
|--------|-------------|----------|
| Basic query params | ?page=1&limit=20&status=active | |
| Full-text search | ?q=search+terms for name and description | |
| Advanced filtering | ?filter[name]=test&filter[tag]=production for structured queries | ✓ |

**User's choice:** Advanced filtering - structured filter queries

---

## Docker Base Image

| Option | Description | Selected |
|--------|-------------|----------|
| Alpine Linux | Minimal size (~5MB); good for security and fast pulls | |
| DebianSlim | Small but includes more libraries; better compatibility | |
| Distroless | No shell/pkg manager; highest security, smallest footprint | ✓ |

**User's choice:** Distroless base image

---

## Health Check Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| /health/live and /health/ready | K8s-style separate endpoints for liveness and readiness | ✓ |
| Single /health endpoint | Combined status check; simpler but less granular | |
| TCP-only health | Just check port 8080 is open - minimal overhead | |

**User's choice:** Separate /health/live and /health/ready endpoints

---

## Ingress Controller

| Option | Description | Selected |
|--------|-------------|----------|
| Nginx Ingress | Most common; good documentation and community | ✓ |
| Traefik | Cloud-native; automatic TLS with Let's Encrypt | |
| AWS ALB / GCP LB | Platform-native load balancers for cloud deployments | |

**User's choice:** Nginx Ingress

---

## Claude's Discretion

- Exact OpenAPI schema details can be finalized during plan implementation
- Specific JWT library choice left to planner/researcher (e.g., jsonwebtoken, go-jose)
- Container registry strategy (Docker Hub, ECR, GCR) deferred to deployment phase

---

## Deferred Ideas

(None - discussion stayed within phase scope)
