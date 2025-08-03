# GitHub Archive Scraper - Rust Edition
# Multi-stage Docker build for production deployment

# ═══════════════════════════════════════════════════════════════════════════════
# Stage 1: Build Environment
# ═══════════════════════════════════════════════════════════════════════════════
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    curl \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN groupadd -r appuser && useradd -r -g appuser appuser

# Set working directory
WORKDIR /app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy source for dependency caching
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src/ src/
COPY static/ static/

# Build application
RUN cargo build --release --bin github_archiver

# Strip binary for smaller size
RUN strip target/release/github_archiver

# ═══════════════════════════════════════════════════════════════════════════════
# Stage 2: Runtime Environment
# ═══════════════════════════════════════════════════════════════════════════════
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create app user
RUN groupadd -r appuser && useradd -r -g appuser -s /bin/bash appuser

# Create application directories
RUN mkdir -p /app/logs /app/gharchive_data /app/static && \
    chown -R appuser:appuser /app

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/github_archiver /usr/local/bin/github_archiver
RUN chmod +x /usr/local/bin/github_archiver

# Copy static files
COPY --from=builder /app/static/ /app/static/

# Copy shell scripts
COPY run.sh setup.sh /app/
RUN chmod +x /app/run.sh /app/setup.sh

# Create environment template
RUN cat > /app/.env.template << 'EOF'
# GitHub Archive Scraper Configuration
# Copy this to .env and customize

# Database Configuration
DB_HOST=postgres
DB_PORT=5432
DB_NAME=github_archiver
DB_USER=github_archiver
DB_PASSWORD=github_archiver_password
DB_MIN_CONNECTIONS=5
DB_MAX_CONNECTIONS=20

# Web Server Configuration
WEB_HOST=0.0.0.0
WEB_PORT=8081

# Download Configuration
DOWNLOAD_DIR=/app/gharchive_data
MAX_CONCURRENT=6
BATCH_SIZE=500
REQUEST_TIMEOUT=180
MAX_RETRIES=3

# Resource Limits
MEMORY_LIMIT_GB=8.0
DISK_LIMIT_GB=50.0
CPU_LIMIT_PERCENT=80.0

# Security Configuration
ADMIN_PASSWORD=admin123
JWT_SECRET=change-this-secret-key

# Logging Configuration
RUST_LOG=info
RUST_BACKTRACE=1
EOF

# Health check script
RUN cat > /app/healthcheck.sh << 'EOF'
#!/bin/bash
set -e

# Check if binary exists and is executable
if [ ! -x "/usr/local/bin/github_archiver" ]; then
    echo "ERROR: github_archiver binary not found or not executable"
    exit 1
fi

# Check if web server is responding
if curl -f -s "http://localhost:${WEB_PORT:-8081}/api/status" > /dev/null; then
    echo "OK: Web server responding"
    exit 0
else
    echo "WARNING: Web server not responding"
    exit 1
fi
EOF

RUN chmod +x /app/healthcheck.sh

# Switch to app user
USER appuser

# Expose ports
EXPOSE 8081

# Environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV WEB_HOST=0.0.0.0
ENV WEB_PORT=8081

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD /app/healthcheck.sh

# Volume for persistent data
VOLUME ["/app/gharchive_data", "/app/logs"]

# Default command
CMD ["/usr/local/bin/github_archiver", "server"]

# ═══════════════════════════════════════════════════════════════════════════════
# Metadata
# ═══════════════════════════════════════════════════════════════════════════════
LABEL maintainer="GitHub Copilot AI"
LABEL version="2.0.0"
LABEL description="GitHub Archive Scraper - High-performance Rust implementation"
LABEL repository="https://github.com/your-repo/github-archiver"

# Build arguments for metadata
ARG BUILD_DATE
ARG VCS_REF
ARG VERSION

LABEL org.label-schema.build-date=$BUILD_DATE
LABEL org.label-schema.name="github-archiver"
LABEL org.label-schema.description="Professional GitHub Archive Processing System"
LABEL org.label-schema.url="https://github.com/your-repo/github-archiver"
LABEL org.label-schema.vcs-ref=$VCS_REF
LABEL org.label-schema.vcs-url="https://github.com/your-repo/github-archiver"
LABEL org.label-schema.vendor="GitHub Copilot AI"
LABEL org.label-schema.version=$VERSION
LABEL org.label-schema.schema-version="1.0"
