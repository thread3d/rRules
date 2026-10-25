# rRules API & Frontend Instructions

## Current Endpoints Available

The rRules server provides the following endpoints:

### Health Checks
| Method | Path | Description |
|--------|------|-------------|
| GET | `/health/live` | Server liveness check (always returns 204) |
| GET | `/health/ready` | Server readiness check |

### API v1 Endpoints
All API endpoints are under the `/v1` prefix.

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/execute` | Execute rules against facts |
| GET | `/v1/rules` | List all rules |
| POST | `/v1/rules` | Create a new rule |
| GET | `/v1/rules/{id}` | Get a specific rule |
| PUT/PATCH | `/v1/rules/{id}` | Update a rule |
| DELETE | `/v1/rules/{id}` | Delete (soft delete) a rule |
| POST | `/v1/rules/reload` | Reload rules from disk |
| GET/POST | `/v1/jobs/*` | Job status and management |
| GET/POST | `/v1/approvals/*` | Approval workflow endpoints |
| GET/POST | `/v1/categories/*` | Category CRUD operations |
| GET/POST | `/v1/tags/*` | Tag CRUD operations |
| GET/POST | `/v1/branches/*` | Branch management |
| GET/POST | `/v1/audit/*` | Audit trail queries |

### Metrics
| Method | Path | Description |
|--------|------|-------------|
| GET | `/metrics` | Prometheus metrics format |

## Quick Start Commands

### Run the API Server
```bash
# From project root
cargo run -p api
```

### Run Frontend Development Server
```bash
cd frontend
npm install  # First time only
npm run dev  # Runs on http://localhost:5173 by default
```

### Build and Preview Frontend
```bash
cd frontend
npm run build    # Creates dist/ directory
npx serve dist   # Serves the built frontend locally
```

## Frontend Configuration

The frontend uses Vite with these key configuration points:

- **Source**: `frontend/src/`
- **Build output**: `frontend/dist/`
- **Vite config**: `frontend/vite.config.ts`
- **Port**: Default 5173 (can be changed with `--port` flag)

### Frontend Environment Variables

Create `.env.development.local` in the frontend directory:
```bash
# API endpoint for development
VITE_API_URL=http://localhost:8080/v1
```

## Adding a Root Route (/) to Serve the Frontend

To serve the frontend at `http://localhost:8080`, modify `api/src/main.rs`:

```rust
use actix_files as fs;

// Add after other app_data configurations, before health endpoints:
.app_data(web::Data::new(job_store.clone()))
// Serve static files from frontend/dist/ at root path
.service(fs::Files::new("/", "./frontend/dist").index_file("index.html"))
// Health check endpoints (must be before /v1 to avoid prefix conflict)
.configure(health::configure_health)
```

Or use the `actix-files` crate with a proper redirect for SPA routing:

```rust
use actix_web::{web, HttpResponse};
use std::fs;

fn index_file() -> impl actix_web::Responder {
    let content = fs::read_to_string("./frontend/dist/index.html")
        .unwrap_or_else(|_| "404 - Frontend not found".to_string());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(content)
}

// In your app configuration:
.route("/", web::get().to(index_file))
```

## Useful Links

### Actix Web Documentation
- [Actix Web 4.4 Docs](https://docs.rs/actix-web/4.4/)
- [Static File Serving](https://actix.rs/docs/files/)

### Frontend Framework Docs
- [Vite Config Reference](https://vitejs.dev/config/)
- [React Router](https://tanstack.com/router/latest/docs/framework/react/overview)
- [TanStack Query](https://tanstack.com/query/latest/docs/react/overview)

### Backend Documentation
- [Tokio Runtime](https://tokio.rs/tokio/tutorial)
- [Serde Serialization](https://serde.rs/)

## Troubleshooting

### Server starts but port is already in use
```bash
# Find process using port 8080
lsof -i :8080
# Or change the bind address in api/src/main.rs
.bind("127.0.0.1:3000")?
```

### Frontend can't connect to API
Check that CORS is enabled in `api/src/main.rs`:
```rust
use actix_cors::Cors;

App::new()
    .wrap(
        Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header(),
    )
```

### Build fails with missing dependencies
```bash
# For frontend
cd frontend && npm install

# For backend
cargo update
cargo build
```
