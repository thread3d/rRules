# rRules Server Helm Chart

A Helm chart for deploying rRules Server to Kubernetes.

## Prerequisites

- Kubernetes 1.24+
- Helm 3+

## Installation

```bash
helm install rrules ./helm/rrules-server
```

## Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of replicas | `2` |
| `image.repository` | Image repository | `rrules/server` |
| `image.tag` | Image tag | `latest` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |
| `securityContext.runAsUser` | Container user ID | `65534` |
| `service.type` | Kubernetes service type | `ClusterIP` |
| `service.port` | Service port | `8080` |
| `resources.requests.memory` | Memory request | `256Mi` |
| `resources.requests.cpu` | CPU request | `100m` |
| `resources.limits.memory` | Memory limit | `512Mi` |
| `resources.limits.cpu` | CPU limit | `500m` |
| `config.RUST_LOG` | Logging level | `info` |
| `config.RULE_STORAGE_PATH` | Rules storage path | `/data/rules` |
| `config.WATCH_RULES_DIR` | Enable file watcher | `true` |

## Usage

### Install with custom values

```bash
helm install rrules ./helm/rrules-server --set replicaCount=3 --set image.tag=9.0.0
```

### Upgrade

```bash
helm upgrade rrules ./helm/rrules-server --values custom-values.yaml
```

### Uninstall

```bash
helm uninstall rrules
```

## Environments

The chart ships per-environment value overlays:

```bash
helm install rrules ./helm/rrules-server -f helm/rrules-server/values-dev.yaml
helm install rrules ./helm/rrules-server -f helm/rrules-server/values-staging.yaml
helm install rrules ./helm/rrules-server -f helm/rrules-server/values-prod.yaml
```

`values.yaml` is the default (ingress off, two replicas). The overlays set their
own replica count, image tag, resources and ingress host, and they turn the
ingress on.

## Secrets

The chart creates the `API_SECRET_KEY` Secret itself (`templates/secret.yaml`).
Precedence: `auth.apiSecretKey` if you set it, else the value already in the
cluster, else a generated 48-character key. The middle case is what keeps an
`helm upgrade` from rotating the key and invalidating every issued token.

Set `auth.apiSecretKey` explicitly in a real deployment:

```bash
helm install rrules ./helm/rrules-server \
  --set auth.apiSecretKey="$(openssl rand -base64 32)"
```

The server's own fallback for an absent key is a public development string, so a
deployment that reaches it can have its tokens forged. `make deploy-env` fails if
any deployment path stops setting it.

`k8s/` holds the equivalent raw manifests for `kubectl apply -k k8s/` when Helm is
not available.
