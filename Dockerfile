# Runtime stage with pre-compiled binary
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy pre-compiled binary and config files
COPY target/release/main /app/gateway
COPY gateway.yaml /app/
COPY api_keys.yaml /app/

EXPOSE 8094

CMD ["./gateway"]
