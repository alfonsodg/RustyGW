# Build stage
FROM rust:1.84-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/rustygw /app/rustygw
COPY gateway.yaml /app/
RUN sed -i 's/127.0.0.1:8094/0.0.0.0:8094/' /app/gateway.yaml
EXPOSE 8094
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -f http://localhost:8094/health || exit 1
CMD ["./rustygw"]
