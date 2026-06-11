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
helm install rrules ./helm/rrules-server --set replicaCount=3 --set image.tag=v0.1.0
```

### Upgrade

```bash
helm upgrade rrules ./helm/rrules-server --values custom-values.yaml
```

### Uninstall

```bash
helm uninstall rrules
```
