# RustyGW

A minimal, high-performance, and self-hosted API Gateway built in Rust. This project provides a lightweight yet powerful solution for managing access to your backend services, perfect for solo developers and small teams.

---

## ✨ Features

- **Dynamic Routing**  
  Configure all routes via a simple YAML file. No code changes or restarts needed to add, remove, or change routes.

- **Reverse Proxy**  
  Forwards client requests to the appropriate backend services seamlessly.

- **Robust Authentication**  
  - **JWT (JSON Web Tokens):** Secure stateless authentication for users.  
  - **API Keys:** Simple, effective authentication for server-to-server communication.  
  - **Role-Based Access Control (RBAC):** Restrict access to specific routes based on roles defined in the JWT or API key data.

- **Rate Limiting**  
  Protect your services from abuse with a configurable Token Bucket algorithm, applied per client IP address.

- **Configuration Hot-Reload**  
  Automatically detects and applies changes to `gateway_config.yaml` and `api_keys.yaml` without any downtime.

- **CLI-Driven**  
  Easy to run and configure via command-line arguments.

---

## 🚀 Getting Started

### Prerequisites

- Rust toolchain (latest stable version recommended)
- Docker (optional, for containerized deployment)

### Quick Start

```bash
# Clone the repository
git clone https://github.com/alfonsodg/Rust-API-Gateway.git
cd Rust-API-Gateway

# Run setup script (installs tools and builds project)
./scripts/setup.sh

# Start the gateway
cargo run
```

### Installation Options

#### Option 1: Run from Source
```bash
cargo run
```

#### Option 2: Install Binary
```bash
cargo install --path .
rustway --config gateway.yaml
```

#### Option 3: Docker
```bash
# Build and run with Docker Compose
docker-compose up

# Or build Docker image manually
docker build -t rustway .
docker run -p 8081:8081 -v $(pwd)/gateway.yaml:/app/gateway.yaml rustway
```

```bash
# To run directly from source
cargo run

# To install the binary
cargo install --path .

⚙️ Configuration
The gateway is configured using three main files:

1. Environment Variables (.env)
This file holds the master secret for the entire gateway and should never be committed to version control.

# .env

# The master secret for signing and verifying all JWTs.
# Use a long, random string for production.
JWT_SECRET="a-very-long-and-random-string-that-is-hard-to-guess"

2. API Key Store (api_keys.yaml)
This file manages all valid API keys and their associated user data and roles.
# api_keys.yaml
keys:
  "user-key-for-alice":
    user_id: "alice@example.com"
    roles: ["user"]
    status: "active"

  "admin-key-for-carol":
    user_id: "carol@example.com"
    roles: ["admin", "user"]
    status: "active"

  "revoked-key-for-dave":
    user_id: "dave@example.com"
    roles: ["user"]
    status: "revoked" # Keys can be easily revoked

3. Main Gateway Config (gateway_config.yaml)
This is the central configuration file that defines the server, routes, and authentication requirements.

# Main server configuration
server:
  addr: "127.0.0.1:8080"

# Defines the location of the API key store
identity:
  api_key_store_path: "./api_keys.yaml"

# --- Route Definitions ---
routes:
  # A public route with no authentication
  - name: "public_service"
    path: "/api/public"
    destination: "http://localhost:9001/some/path"

  # A route protected by an API key requiring the 'user' role
  - name: "user_service"
    path: "/api/user"
    destination: "http://localhost:9002"
    auth:
      type: "apikey"
      roles: ["user"]
    rate_limit:
      requests: 10
      period: "1m" # 10 requests per minute

  # A route protected by a JWT requiring the 'admin' role
  - name: "admin_dashboard"
    path: "/api/admin"
    destination: "http://localhost:9003"
    auth:
      type: "jwt"
      roles: ["admin"]

▶️ Running the Gateway

Default (uses gateway_config.yaml in the current directory)
cargo run

With a Custom Config File
cargo run -- --config /path/to/your/custom_config.yaml
# OR
cargo run -- -c /path/to/your/custom_config.yaml

---

## 📊 Monitoring & Observability

The gateway provides built-in metrics and monitoring capabilities:

### Metrics Endpoint
- **URL**: `http://localhost:8081/metrics`
- **Format**: Prometheus-compatible metrics
- **Includes**: Request counts, response times, error rates, rate limiting stats

### Prometheus Integration
```bash
# Use the provided Prometheus configuration
prometheus --config.file=examples/prometheus.yml
```

### Docker Monitoring Stack
```bash
# Run gateway with monitoring
docker-compose up
```

### Health Checks
- **Endpoint**: `http://localhost:8081/health`
- **Docker**: Built-in health check configured
- **Kubernetes**: Ready for liveness/readiness probes

---

## 🔧 Development

### Development Tools
```bash
# Auto-reload on file changes
cargo watch -x run

# Run security audit
cargo audit

# Format code
cargo fmt

# Lint code
cargo clippy
```

---

## 🧪 Testing

The project includes a comprehensive test suite with 15+ tests covering:

### Test Suites
- **Configuration Tests**: YAML parsing and validation
- **Rate Limiting Tests**: Token bucket algorithms and time windows
- **Concurrency Tests**: Thread safety and race conditions
- **Integration Tests**: End-to-end gateway functionality

### Running Tests
```bash
# Run all tests
cargo test

# Run specific test suite
cargo test --test auth_test
cargo test --test rate_limit
cargo test --test concurrency_tests

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Performance Benchmarks
- **Route Matching**: 65M+ operations/second
- **String Operations**: Optimized path processing
- **Rate Limiting**: 1.6G+ token acquisitions/second
- **Cache Operations**: High-performance key operations

---

## 🚀 Production Deployment

### Docker Production
```bash
# Build optimized image
docker build -t rustway:latest .

# Run in production
docker run -d \
  --name rustway \
  -p 8081:8081 \
  -v /path/to/config:/app/gateway.yaml:ro \
  --restart unless-stopped \
  rustway:latest
```

### Kubernetes
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rustway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rustway
  template:
    metadata:
      labels:
        app: rustway
    spec:
      containers:
      - name: rustway
        image: rustway:latest
        ports:
        - containerPort: 8081
        livenessProbe:
          httpGet:
            path: /metrics
            port: 8081
          initialDelaySeconds: 30
          periodSeconds: 10
```

---

## 📈 Performance

- **Throughput**: 10,000+ requests/second
- **Latency**: Sub-millisecond routing overhead
- **Memory**: ~10MB base footprint
- **CPU**: Minimal overhead with async processing
- **Benchmarks**: Included performance test suite

---

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Run tests: `cargo test`
4. Run lints: `cargo clippy`
5. Format code: `cargo fmt`
6. Submit a pull request

---

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.