# RustyGW Architecture

## Overview

RustyGW is a high-performance API gateway built with Rust, Axum, and Tokio.
It provides reverse proxying, load balancing, authentication, rate limiting,
caching, circuit breaking, and real-time WebSocket/gRPC support.

## Request Flow

    Client Request
         |
         v
    +--------------------+
    |   Axum Router      |  Route matching (matchit tree)
    +--------------------+
         |
         v
    +--------------------+
    |   Middleware Stack  |  Sequential processing
    |  1. Request ID     |  Assign unique trace ID
    |  2. Access Log     |  Log request metadata
    |  3. Tracing Ctx    |  W3C Trace Context propagation
    |  4. Rate Limiter   |  Token bucket per IP/route
    |  5. Auth           |  JWT or API key validation
    |  6. Circuit Breaker|  Fail-fast on unhealthy backends
    |  7. Cache          |  Response cache with TTL
    +--------------------+
         |
         v
    +--------------------+
    |   Handler          |  Based on route type:
    |  - proxy_handler   |  HTTP reverse proxy
    |  - ws_proxy        |  WebSocket bidirectional
    |  - grpc_proxy      |  HTTP/2 transparent proxy
    |  - aggregate       |  Multi-service composition
    +--------------------+
         |
         v
    +--------------------+
    |   Load Balancer    |  Round-robin or random
    |   Health Checker   |  Filter unhealthy backends
    +--------------------+
         |
         v
    Backend Service(s)

## Core Components

### Config (src/config.rs)

- YAML-based configuration with environment variable interpolation
- Service abstraction: define backend pools once, reference in routes
- Global defaults: timeout, retry, load balance strategy
- Route tree built with `matchit` for segment-aware path matching
- Hot-reload via filesystem watcher (zero-downtime config updates)

### Proxy (src/proxy.rs)

- HTTP reverse proxy using `reqwest` client
- Path parameter substitution from route templates
- Request/response header transformation
- Configurable retry with exponential backoff
- Per-route timeout and TLS skip-verify options

### Load Balancer (src/features/load_balancer.rs)

- Round-robin and random strategies
- Returns `Option<usize>` to safely handle empty backend pools
- Thread-safe via `AtomicUsize` counter

### Health Checker (src/features/health_check.rs)

- Active health checks with configurable interval and path
- Automatic backend removal on failure
- Fallback to all backends when all are marked unhealthy

### Authentication (src/features/auth/)

- JWT token validation with role-based access control
- API key authentication with per-key role assignment
- Per-route auth configuration (type, required roles)

### Rate Limiter (src/features/rate_limiter/)

- Token bucket algorithm per client IP
- Configurable requests/period per route
- DashMap for concurrent access

### Circuit Breaker (src/middleware/circuit_breaker/)

- Three states: Closed, Open, HalfOpen
- Configurable failure threshold and recovery timeout
- Per-route circuit breaker instances

## Deployment Architecture

    +------------------+
    |   Nginx          |  SSL termination, static files
    |   (reverse proxy)|
    +------------------+
           |
           v
    +------------------+
    |   RustyGW        |  Port 8094
    |   (gateway)      |  /health, /metrics
    +------------------+
        |      |      |
        v      v      v
    +------+ +------+ +------+
    | Svc1 | | Svc2 | | Svc3 |  Backend services
    +------+ +------+ +------+

    Monitoring: Prometheus scrapes /metrics -> Grafana dashboards

## Configuration Example

    server:
      addr: "0.0.0.0:8094"
      pool:
        max_idle: 100
        idle_timeout: 90s

    services:
      users:
        urls: ["http://localhost:9001", "http://localhost:9002"]
        load_balance: round_robin

    routes:
      - name: users-api
        path: /api/users
        service: users
        auth:
          type: ApiKey
          roles: [user]
        rate_limit:
          requests: 100
          period: 1m

## Performance

- 20,000+ requests/sec sustained (wrk benchmark)
- Sub-10ms average latency under load
- 8.5MB optimized binary
- Async I/O via Tokio runtime
