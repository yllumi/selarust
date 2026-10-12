# Sayagi Rust — Architecture

## 1. Overview & Vision

Sayagi Rust adalah fullstack PWA metaframework khusus mobile-first yang dibangun di atas bahasa pemrograman Rust. Metaframework ini bertujuan untuk memberikan pengalaman pengembang (Developer Experience) secepat dan semudah Sayagi versi PHP (Webman + Alpine.js), namun membawa performa, type-safety, serta memory-efficiency tingkat lanjut menggunakan Axum dan Leptos.

### Pemilihan Core Framework

**Backend & Server Engine: Axum**

- **Alasan:** Dibuat oleh tim Tokio (tokio-async runtime), Axum adalah standar industri untuk backend Rust yang modular, cepat, dan memiliki ekosistem middleware (Tower) yang sangat kuat.

**Frontend & SSR Engine: Leptos**

- **Alasan:** Leptos mendukung fine-grained reactivity (tanpa Virtual DOM), kompilasi frontend murni ke WebAssembly (Wasm), serta memiliki fitur Server Functions yang memungkinkan pemanggilan fungsi server langsung dari frontend layaknya fungsi lokal.

## 2. System Architecture

```text
                                 [ Browser / Mobile PWA ]
                                            │
                     ┌──────────────────────┴──────────────────────┐
                     │                                             │
             [ Wasm Client Runtime ]                        [ Service Worker ]
             • Fine-grained UI (Leptos)                     • Offline Cache
             • Mobile UI Shell & Gestures                   • Background Sync
                     │                                             │
                     └──────────────────────┬──────────────────────┘
                                            │ (HTTP/2 / WebSockets / Server Functions)
                                            ▼
                                   [ Axum Server Core ]
                                            │
             ┌──────────────────────────────┼──────────────────────────────┐
             │                              │                              │
     [ Page Router Engine ]        [ Auto-PWA Generator ]         [ Embedded SQLite ]
     • File-based Routing          • Manifest.json                • WAL Mode (rusqlite/sqlx)
     • SSR & Hydration             • SW Registration              • Local Caching/Storage
```

## 3. Core Capabilities & Mechanics

### A. Page-Based Routing (Sayagi Adaptation)

Mengadaptasi pola perutean halaman ala Sayagi, struktur folder `src/pages/` secara otomatis dipetakan menjadi rute PWA dan HTTP endpoint:

```text
src/
├── pages/
│   ├── index.rs            --> Rute: "/"
│   ├── dashboard.rs        --> Rute: "/dashboard"
│   └── containers/
│       ├── [id].rs         --> Rute: "/containers/:id"
│       └── index.rs        --> Rute: "/containers"
├── components/             --> Reusable UI Components
├── service_worker.rs       --> PWA Service Worker Logic (compiled to Wasm)
└── main.rs                 --> Application Entrypoint
```

### B. Seamless Data Fetching (Server Functions)

Menggantikan kebutuhan penulisan manual REST API endpoint untuk komunikasi internal. Cukup buat fungsi beranotasi `#[server]`:

```rust
#[server(GetContainers, "/api")]
pub async fn get_containers() -> Result<Vec<Container>, ServerFnError> {
    // Kode ini berjalan murni di server (Axum)
    let db = use_context::<SqlitePool>()?;
    let data = sqlx::query_as!(Container, "SELECT id, name, status FROM containers").fetch_all(&db).await?;
    Ok(data)
}
```

### C. PWA First-Class Support

Metaframework menangani siklus hidup PWA secara otomatis di belakang layar:

- **Automatic Manifest Generation:** Generasi file `manifest.webmanifest` secara terintegrasi berdasarkan konfigurasi `Sayagi.toml`.
- **Embedded Service Worker Engine:** Kompilasi Service Worker ke Wasm/JS dengan strategi caching Network-First untuk API data dan Stale-While-Revalidate untuk bundle Wasm frontend.
- **Mobile UI Shell Components:** Menyediakan primitives bawaan seperti `MobileLayout`, `BottomNav`, `HeaderBar`, dan pull-to-refresh container.

## 4. Tech Stack Breakdown

| Layer | Technology | Role |
| --- | --- | --- |
| Language | Rust (Edition 2021+) | Unifying language for Server and Wasm Client |
| Server Framework | Axum | HTTP Server, Routing, Middleware, WebSocket Handling |
| Frontend Framework | Leptos | Reactive Web Components, SSR, Wasm Hydration |
| Database Engine | SQLite (WAL Mode) | Embedded database using sqlx or rusqlite |
| Async Runtime | Tokio | Non-blocking I/O execution engine |
| Build Tooling | cargo-leptos / Custom CLI | Zero-config compilation pipeline to Wasm & Binary |

## 5. Development Workflow & CLI

Proses pengembangan dirancang agar sedekat mungkin dengan kesederhanaan CLI Sayagi:

### Inisialisasi Proyek Baru

```bash
sayagi-cli new my-pwa-app
```

### Menjalankan Mode Development (Hot-Reload)

```bash
sayagi-cli dev
```

Mengompilasi backend Axum dan Wasm frontend secara simultan dengan fitur auto-reload.

### Build Production (Single Executable Binary + Wasm Assets)

```bash
sayagi-cli build --release
```

## 6. Next Steps & Roadmap

- [ ] **Phase 1:** Implementasi dasar File-based router macro untuk Leptos + Axum.
- [ ] **Phase 2:** Integrasi otomatis Service Worker & PWA Manifest generator.
- [ ] **Phase 3:** Pembuatan modul Mobile UI Shell (Gestures, Page Transitions, Bottom Sheet).
- [ ] **Phase 4:** Pengujian performa offline-storage & sinkronisasi data via WebSocket.

## 7. Status Implementasi di Repo Ini

Boilerplate yang menyertai dokumen ini sudah menjalankan sebagian besar rencana di atas.

| Fase | Status | Wujudnya |
| --- | --- | --- |
| Phase 1 — File-based router | ✅ | [`sayagi-build/src/routes.rs`](./crates/sayagi-build/src/routes.rs) memindai `src/pages` saat build dan menghasilkan `$OUT_DIR/routes.rs`: deklarasi modul `#[path]`, tabel `ROUTES`, dan komponen `FileRoutes`. Dikerjakan sebagai codegen build script (bukan proc macro) supaya nama berkas seperti `[id].rs` bisa dipakai apa adanya. |
| Phase 2 — Service Worker & Manifest generator | ✅ | [`sayagi-build/src/pwa.rs`](./crates/sayagi-build/src/pwa.rs) menulis `manifest.webmanifest`, `sw.js` (network-first `/api/`, stale-while-revalidate `/pkg/`, fallback `/offline`), dan ikon PNG 192/512 — semuanya dari `Sayagi.toml`. |
| Phase 3 — Mobile UI Shell | ⚠️ sebagian | `MobileLayout`, `HeaderBar`, `BottomNav`, dan `PullToRefresh` sudah tersedia di crate [`sayagi`](./crates/sayagi/src/components.rs). Page transitions dan bottom sheet belum dikerjakan. |
| Phase 4 — Uji offline & sinkronisasi | ⚠️ sebagian | Caching offline, halaman `/offline`, dan banner konektivitas sudah berjalan. Kanal WebSocket (`/api/ws`) tersedia sebagai titik awal sinkronisasi, tetapi protokol dan uji beban belum ada. |

Struktur folder, konvensi halaman, dan cara menjalankan ada di [README.md](./README.md).
