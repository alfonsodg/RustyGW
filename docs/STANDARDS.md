# RustyGW Development Standards

## Language and Tooling

- Rust edition 2024, stable toolchain 1.84+
- Formatter: `rustfmt` (default configuration)
- Linter: `clippy` with `-D warnings`
- Security: `cargo audit` enforced before every release
- Dependencies: latest stable versions, no beta/alpha

## Error Handling

- No `unwrap()` or `expect()` in runtime request paths
- Startup-only panics are acceptable for configuration errors
- Use `Result<T, AppError>` for all fallible proxy/middleware operations
- Log errors with structured context: route name, method, status, destination

## Code Organization

    src/
    +-- main.rs           Entry point
    +-- lib.rs             App builder and HTTP client setup
    +-- app.rs             Axum router and middleware stack
    +-- config.rs          Config loading, validation, route tree
    +-- errors.rs          AppError enum and IntoResponse
    +-- proxy.rs           HTTP reverse proxy handler
    +-- ws_proxy.rs        WebSocket proxy handler
    +-- grpc_proxy.rs      gRPC transparent proxy (HTTP/2)
    +-- aggregate.rs       Multi-service response aggregation
    +-- state.rs           Shared application state
    +-- features/          Core features (auth, CB, health, LB)
    +-- middleware/         Axum middleware layers
    +-- utils/             Hot reload, metrics, config path helpers
    +-- plugins/           Plugin system (experimental)

## Testing

- Unit tests for pure logic (config parsing, load balancer math, rate limit)
- Integration tests for middleware behavior and error paths
- Negative tests for all failure scenarios (empty backends, bad config, auth rejection)
- Run: `cargo test`

## Commits

- Conventional Commits: `<type>(<scope>): <subject> (#issue)`
- Types: feat, fix, docs, test, chore, refactor, style
- Every commit references an issue number
- Use `Closes #N` in final commit for auto-close

## Configuration

- Gateway config: `gateway.yaml` (YAML with env var interpolation)
- API keys: `api_keys.yaml` (never tracked in git)
- Safe example: `api_keys.example.yaml`
- Port: 8094 (canonical across all deployment artifacts)

## Security

- No secrets in git (REMOTE.md, api_keys.yaml excluded via .gitignore)
- `cargo audit` must pass with zero vulnerabilities
- API keys mounted at runtime, never baked into Docker images
- TLS certificate validation enabled by default (opt-out per route)

## Docker

- Multi-stage Dockerfile (build from source)
- Runtime image: debian:bookworm-slim with ca-certificates
- Health check on `/health` endpoint
- Secrets injected via volume mounts, not COPY
