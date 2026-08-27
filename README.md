# 🌌 AeroFS — Modern & Ultra-Fast Web File Manager

<p align="center">
  <b>A modern, high-performance, and self-hosted cloud & local file manager powered by Rust (Axum) and Vue 3 + Tailwind CSS.</b>
</p>

---

## ⚡ Key Features

- **🚀 Blazing Fast Backend (Rust + Axum + Tokio)**:
  - Asynchronous streaming architecture for gigabyte-scale uploads and downloads.
  - SQLite WAL mode database for fast metadata queries, audit logging, and share links.
  - SafePath filesystem isolation preventing path traversal attacks and escaping symlinks.

- **🎨 Modern & Responsive Frontend (Vue 3 + Vite + Tailwind CSS 4)**:
  - **Dual-Pane & Single-Pane Workspace**: Work seamlessly across multiple local and remote directories side-by-side.
  - **Light & Dark Theme**: Adaptive design with modern rounded cards and soft contrast.
  - **Integrated Ace Code Editor**: Instant syntax-highlighted editing for code, scripts, configs, and dotfiles (`.env`, `.gitignore`, `.bashrc`, etc.) with `Ctrl+S` saving.
  - **Live Storage Statistics**: Real-time disk capacity and usage metrics for local storage (`statvfs`) and live status indicators for remote storage.
  - **Recycle Bin (Soft Delete)**: Safety first with a choice between moving to Recycle Bin or permanent deletion.
  - **Properties & CHMOD Inspector**: 3x3 Unix permission matrix with 2-way octal sync (`0755`/`0644`) and recursive apply.
  - **Multi-Source Support**: Built-in support for Local Storage, Remote FTP, SFTP, and S3 connections.
  - **Rich Context Menu & Starred Items**: Quick actions for compression (`.zip`, `.tar.gz`), instant extraction, sharing links, renaming, and bookmarking.

---

## 📁 Monorepo Structure

```text
aerofs/
├── backend/                  # High-performance Rust Axum API Service
│   ├── Cargo.toml            # Dependencies (Axum, Tokio, SQLx, SuppaFTP, Utioipa)
│   ├── migrations/           # SQLite schema migrations
│   └── src/
│       ├── api/              # RESTful API endpoints (files, trash, chmod, stats)
│       ├── auth/             # Argon2 hashing, session tokens, audit logging
│       ├── domain/           # VFS abstractions and entities
│       ├── filesystem/       # SafePath security, zip/tar engines, fast search
│       └── router.rs         # Axum route definitions
│
├── frontend/                 # Reactive Vue 3 + Vite Single Page Application
│   ├── package.json          # Node dependencies (Vue 3, Pinia, Tailwind, Ace)
│   ├── src/
│   │   ├── api/              # Axios API clients
│   │   ├── components/       # UI components (browser, dialogs, layout, editor)
│   │   └── stores/           # Pinia reactive state stores
│   └── index.html            # Entry HTML
│
└── README.md                 # Project documentation
```

---

## 🚀 Quickstart Guide

### Prerequisites
- **Rust toolchain** (1.80+): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Node.js** (18+) & **npm**: `https://nodejs.org/`

### 1. Run Backend (Rust API)
```bash
cd backend
cargo run
```
*The backend API server will start at `http://127.0.0.1:8080` (or `http://0.0.0.0:8080`).*

### 2. Run Frontend (Vue 3 Dev Server)
```bash
cd frontend
npm install
npm run dev
```
*The web interface will be available at `http://localhost:5173`.*

---

## 🔐 Default Login Credentials

| Username | Password | Role |
| :--- | :--- | :--- |
| `admin` | `admin123` | **Administrator** |

*(You can change default credentials and configure custom storage paths in System Settings).*

---

## 🛠️ Building for Production

### Build Frontend
```bash
cd frontend
npm run build
```
*Outputs minified static production bundle in `frontend/dist/`.*

### Build Backend
```bash
cd backend
cargo build --release
```
*Outputs optimized native binary in `backend/target/release/backend`.*

---

## 📄 License
MIT License. Open source and self-hosted friendly.
