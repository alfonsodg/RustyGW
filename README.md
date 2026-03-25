# RustyGW

A high-performance internal API Gateway for microservices. Designed for backend-to-frontend (BTF) and backend-to-backend (BTB) communication within a service stack.

> Built upon [Rust-API-Gateway](https://github.com/Ketankhunti/Rust-API-Gateway) by [@Ketankhunti](https://github.com/Ketankhunti), extended with load balancing, WebSocket/gRPC proxy, API composition, health checks, distributed tracing, and production-grade resilience.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92+-orange.svg)](https://www.rust-lang.org)
[![Release](https://img.shields.io/github/v/release/alfonsodg/RustyGW)](https://github.com/alfonsodg/RustyGW/releases)

---

## Features

### Proxy
- **HTTP Proxy** with path-based routing
- **WebSocket Proxy** (`/ws/`) for real-time BTF communication
- **gRPC Proxy** (`/grpc/`) with HTTP/2 transparent forwarding
- **API Composition** (`/agg/`) — fan-out 1 request to N backends, merge responses

### Resilience
- **Load Balancing** — round-robin, random across multiple destinations
- **Active Health Checks** — periodic probes, auto-remove/recover backends
- **Retry + Timeout** — per-route retry count, backoff, status codes, timeout
- **Circuit Breaker** — fault tolerance with configurable thresholds

### Transformation
- **Path Rewriting** — rewrite request paths with `{path}` placeholder
- **Header Injection/Removal** — add or remove request and response headers
- **Response Compression** — automatic gzip

### Observability
- **Prometheus Metrics** — request count, latency histograms, error rates
- **W3C Distributed Tracing** — auto-generate and propagate `traceparent`
- **Structured Access Logs** — method, path, status, duration_ms per request
- **Health Endpoint** — `GET /health` returns `OK`

### Security
- **JWT + API Key Authentication** with RBAC
- **Rate Limiting** — per-IP (BTF) or per-service via `x-service-name` header (BTB)
- **CORS** — configurable origins, methods, headers
- **TLS Skip Verify** — per-route flag for self-signed backend certs
- **Body Size Limits** — configurable max request body

### Operations
- **Hot Reload** — zero-downtime config updates
- **Connection Pooling** — configurable idle timeout, max connections
- **Docker Swarm** — production cluster with replicas and health checks
- **9.8MB Binary** — single executable, no dependencies

---

## Quick Start

```bash
# From source
git clone https://github.com/alfonsodg/RustyGW.git
cd RustyGW
cargo build --release
./target/release/rustygw

# Or Docker
cd demo && docker-compose up
```

---

## Configuration

### Full Example (`gateway.yaml`)

```yaml
server:
  addr: "0.0.0.0:8094"
  pool:
    idle_timeout: 90s
    max_idle_per_host: 32
    connect_timeout: 5s
    request_timeout: 30s
    body_limit: 10mb

cors:
  enabled: true
  origins: ["https://app.example.com"]
  methods: [GET, POST, PUT, DELETE, PATCH, OPTIONS]
  allow_headers: [content-type, authorization]

observability:
  metrics:
    enabled: true

identity:
  api_key_store_path: "./api_keys.yaml"

routes:
  # Simple proxy
  - name: users
    path: /api/users
    destination: http://users-service:8091/users

  # Load balanced with health checks
  - name: payments
    path: /api/payments
    destination: http://payments-1:8080
    destinations:
      - http://payments-1:8080
      - http://payments-2:8080
      - http://payments-3:8080
    load_balance: round_robin
    health_check:
      interval: 5s
      path: /health

  # Retry + timeout
  - name: orders
    path: /api/orders
    destination: http://orders-service:8093
    timeout: 5s
    retry:
      count: 2
      backoff: 100ms
      retry_on: [502, 503, 504]

  # Request/response transformation
  - name: legacy_api
    path: /api/v2/products
    destination: http://products-service:8092
    transform:
      rewrite_path: /api/v1{path}
      request_headers:
        x-api-version: "2"
      response_headers:
        x-powered-by: RustyGW
      remove_response_headers: [server]

  # API composition (BTF killer feature)
  - name: dashboard
    path: /api/dashboard
    destination: http://localhost
    aggregate:
      - service: users
        path: http://users-service:8091/me
        field: user
        timeout: 3s
      - service: orders
        path: http://orders-service:8093/recent
        field: orders
      - service: notifications
        path: http://notifications:8095/unread
        field: notifications

  # WebSocket route
  - name: live
    path: /api/live
    destination: http://notifications:8095/ws

  # HTTPS backend with self-signed cert
  - name: internal_secure
    path: /api/secure
    destination: https://internal-service:8443
    tls_skip_verify: true

  # Auth + rate limiting
  - name: admin
    path: /admin
    destination: http://admin-service:9000
    auth:
      type: ApiKey
      roles: [admin]
    rate_limit:
      requests: 100
      period: 1m
```

---

## Usage Patterns

### BTF (Backend-to-Frontend)

Frontend calls RustyGW as its single entry point:

```
Browser → RustyGW:8094 → Backend services
```

Key features: API composition, CORS, WebSocket proxy, gzip compression, rate limiting by IP.

```bash
# 1 call = data from 3 services
curl http://gateway:8094/agg/api/dashboard
# → {"user": {...}, "orders": [...], "notifications": [...]}

# WebSocket through gateway
wscat -c ws://gateway:8094/ws/api/live
```

### BTB (Backend-to-Backend)

Services call RustyGW instead of calling each other directly:

```
Service A → RustyGW:8094 → Service B (with LB, retry, health checks)
```

Key features: load balancing, health checks, retry/timeout, circuit breaker, rate limiting by service name.

```bash
# Service A calls through gateway with service identity
curl -H "x-service-name: order-service" http://gateway:8094/api/payments
```

---

## Endpoints

| Path | Protocol | Description |
|------|----------|-------------|
| `/{path}` | HTTP | Standard proxy with auth/rate-limit middleware |
| `/ws/{path}` | WebSocket | Bidirectional WebSocket proxy |
| `/agg/{path}` | HTTP | API composition (fan-out + merge) |
| `/grpc/{path}` | gRPC/HTTP2 | Transparent gRPC proxy |
| `/health` | HTTP | Health check (`OK`) |
| `/metrics` | HTTP | Prometheus metrics |

---

## Performance

| Metric | Value |
|--------|-------|
| Throughput | 20,000+ req/sec |
| Avg Latency | 4.59ms (100 connections) |
| Max Latency | 41.64ms under load |
| Binary Size | 9.8MB |
| Memory | ~10MB footprint |

See [PERFORMANCE.md](PERFORMANCE.md) for detailed wrk benchmark results.

---

## Production Deployment

### Docker Swarm

```bash
docker swarm init
docker stack deploy -c docker-compose.swarm.yml rustygw
docker service scale rustygw_gateway=3
```

### Architecture with Traefik

```
Internet → Traefik (TLS/edge) → Services
                                    ↓
                          Service A → RustyGW → Service B (BTB)
                          Frontend  → RustyGW → Backends  (BTF)
```

RustyGW sits behind Traefik inside the Swarm overlay network. Traefik handles external TLS, RustyGW handles internal routing, load balancing, and resilience.

---

## Development

```bash
cargo build          # Build
cargo test           # Run tests
cargo clippy         # Lint
cargo audit          # Security audit
cargo watch -x run   # Hot reload dev
```

---

## Acknowledgments

Built upon [Rust-API-Gateway](https://github.com/Ketankhunti/Rust-API-Gateway) by [@Ketankhunti](https://github.com/Ketankhunti). Extended with load balancing, WebSocket/gRPC proxy, API composition, health checks, distributed tracing, CORS, compression, and production-grade resilience patterns.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
