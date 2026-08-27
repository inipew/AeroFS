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
# Stage 2: Build Rust Backend Binary with Embedded Frontend
# ==========================================
FROM rust:1.85-slim AS backend-builder
WORKDIR /app/backend

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock* ./
COPY backend/migrations ./migrations

# Copy compiled frontend dist so rust-embed embeds it into the single standalone binary
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Copy real source code and build final release binary
COPY backend/src ./src
RUN cargo build --release

# ==========================================
# Stage 3: Minimal Production Runtime (100% Single Standalone Binary)
# ==========================================
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates sqlite3 curl && rm -rf /var/lib/apt/lists/*

# Copy single self-contained backend binary (with embedded frontend)
COPY --from=backend-builder /app/backend/target/release/backend /app/backend

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
