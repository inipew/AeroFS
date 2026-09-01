# 🌌 AeroFS — Modern & Ultra-Fast Web File Manager

<p align="center">
  <b>A modern, high-performance, and self-hosted cloud & local file manager powered by Rust (Axum) and Vue 3 + Tailwind CSS.</b><br>
  <i>Bundled into a single self-contained standalone binary with an integrated CLI operational control plane.</i>
</p>

---

## ⚡ Key Features

- **🚀 Single Self-Contained Binary (Rust + Vue 3 SPA Embedded)**:
  - Frontend SPA (HTML, JS, CSS, SVG icons, Ace Editor) is embedded directly into the Rust executable binary via `rust-embed`.
  - Zero external web server or Node.js runtime required in production — run just `./backend` or `aerofs serve`.

- **🖥️ Operational CLI Control Plane (`aerofs`)**:
  - Full CLI management subcommands: `serve`, `config`, `doctor`, `db`, `transfer`, `admin`.
  - Output formatting in human-readable table or machine-readable JSON (`--json`) for CI/CD automation.

- **⚙️ Hierarchical Configuration Loader**:
  - Prioritized configuration resolution: **CLI Arguments** $\rightarrow$ **Environment Variables (`AEROFS_*`)** $\rightarrow$ **TOML Configuration File (`/etc/aerofs/config.toml` or `--config`)** $\rightarrow$ **Defaults**.
  - Automatic secrets sanitization and production mode security enforcement.

- **🛡️ High-Performance Multi-Cloud VFS & SQLite WAL**:
  - Unified Virtual File System powered by OpenDAL supporting **Local Storage**, **AWS S3 / MinIO**, **FTP**, and **SFTP**.
  - **SQLite WAL Mode** (`PRAGMA journal_mode = WAL;`) for high-concurrency read/write transactions without lock contention.
  - SafePath filesystem isolation preventing path traversal attacks and escaping symlinks.

- **🎨 Modern & Responsive Dual-Pane Frontend (Vue 3 + Vite + Tailwind CSS)**:
  - **Dual-Pane & Single-Pane Workspace**: Side-by-side file operations with drag-and-drop and touch swipe gestures (`swipe left/right`) on mobile.
  - **Smart Navigation Header**: Direct path editing with `Ctrl+L`, popover breadcrumb truncation, quick bookmarking, and search palette (`⌘K`).
  - **Durable Transfer Manager**: SQLite-backed background transfer queue with pause, resume, cancel, auto-retry, and WebSocket live progress tracking.
  - **Integrated Ace Code Editor**: Syntax-highlighted editing for code, scripts, and dotfiles (`.env`, `.gitignore`, `.bashrc`) with `Ctrl+S` saving.
  - **Recycle Bin & Unix CHMOD Inspector**: 3x3 Unix permission matrix with 2-way octal sync (`0755`/`0644`) and soft-delete capabilities.

---

## 📁 Monorepo Structure

```text
aerofs/
├── backend/                  # High-performance Rust Axum API Service & CLI
│   ├── Cargo.toml            # Dependencies (Axum, Clap, SQLx, OpenDAL, Utioipa, rust-embed)
│   ├── migrations/           # SQLite schema migrations
│   └── src/
│       ├── api/              # RESTful API endpoints (files, trash, transfers, shares, settings)
│       ├── auth/             # Argon2 hashing, session tokens, audit logging
│       ├── cli.rs            # Operational CLI subcommands (serve, doctor, db, config, admin)
│       ├── config.rs         # Hierarchical TOML & environment configuration loader
│       ├── domain/           # VFS abstractions and entities
│       ├── filesystem/       # SafePath security, zip/tar engines, fast search
│       ├── static_files.rs   # Embedded SPA static asset handler
│       ├── transfer/         # Durable SQLite-backed background transfer engine
│       └── router.rs         # Axum route definitions
│
├── frontend/                 # Reactive Vue 3 + Vite Single Page Application (Bun)
│   ├── package.json          # Frontend dependencies (Vue 3, Pinia, Tailwind, Ace)
│   ├── bun.lock              # Bun lockfile
│   ├── bunfig.toml           # Bun project configuration
│   ├── src/
│   │   ├── api/              # Axios API clients & WebSocket client
│   │   ├── components/       # UI components (browser, dialogs, layout, editor, header)
│   │   └── stores/           # Pinia reactive state stores
│   └── index.html            # Entry HTML
│
├── config.example.toml       # Example TOML configuration file
├── Dockerfile                # Multi-stage build producing 1 single binary container
└── README.md                 # Project documentation
```

---

## 🚀 Quickstart Guide

### 1. Build and Run (Single Standalone Binary)
```bash
# 1. Build Vue 3 Frontend with Bun
cd frontend
bun install
bun run build

# 2. Build Release Rust Binary with Embedded Frontend
cd ../backend
cargo build --release

# 3. Run AeroFS Server
./target/release/backend serve
```
*Open your browser at `http://127.0.0.1:8080` to access the Web File Manager.*

---

## 🖥️ CLI Management Commands

AeroFS includes a comprehensive CLI control plane:

```bash
# Check system health & diagnostics
aerofs doctor

# Inspect active configuration (secrets masked)
aerofs config show
aerofs config show --json

# Database maintenance & online backups
aerofs db status
aerofs db integrity-check
aerofs db vacuum
aerofs db backup /path/to/backup.db

# Inspect background transfers
aerofs transfer list --active
aerofs transfer purge --days 30

# Manage users from the terminal
aerofs admin user list
aerofs admin user create operator --password secretpass --admin
aerofs admin user reset-password operator --password newsecretpass
```

---

## 🔐 Default Login Credentials

| Username | Password | Role |
| :--- | :--- | :--- |
| `admin` | `admin12345` | **Administrator** |

*(You can customize the initial password with `AEROFS_ADMIN_PASSWORD` or reset it via `aerofs admin user reset-password`).*

---

## 📄 License
MIT License. Open source and self-hosted friendly.
