# Runtime stage with pre-compiled binary
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy pre-compiled binary and config files
COPY target/release/rustygw /app/gateway
COPY gateway.yaml /app/
COPY api_keys.yaml /app/

# Override bind address for container networking
RUN sed -i 's/127.0.0.1:8094/0.0.0.0:8094/' /app/gateway.yaml

EXPOSE 8094

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -f http://localhost:8094/metrics || exit 1

CMD ["./gateway"]
