# ==========================================
# Stage 1: Build Vue 3 Frontend SPA
# ==========================================
FROM node:22-alpine AS frontend-builder
WORKDIR /app/frontend

COPY frontend/package*.json ./
RUN npm install

COPY frontend/ ./
RUN npm run build

# ==========================================
# Stage 2: Build Rust Backend Binary
# ==========================================
FROM rust:1.85-slim AS backend-builder
WORKDIR /app/backend

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock* ./
COPY backend/migrations ./migrations

# Cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "pub fn lib() {}" > src/lib.rs
RUN cargo build --release || true
RUN rm -rf src

# Copy real source code and build final binary
COPY backend/src ./src
RUN cargo build --release

# ==========================================
# Stage 3: Minimal Production Runtime
# ==========================================
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates sqlite3 curl && rm -rf /var/lib/apt/lists/*

# Copy backend binary
COPY --from=backend-builder /app/backend/target/release/backend /app/backend

# Copy compiled frontend static assets
COPY --from=frontend-builder /app/frontend/dist /app/static

# Create data and storage directories
RUN mkdir -p /app/data /app/storage

ENV APP_SERVER__PORT=8080 \
    APP_DATABASE__URL="sqlite:///app/data/filemanager.db?mode=rwc" \
    APP_FILESYSTEM__DEFAULT_LOCAL_ROOT="/app/storage" \
    RUST_LOG="backend=info,tower_http=info"

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s \
  CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["/app/backend"]
