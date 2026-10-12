# Sayagi Rust — boilerplate PWA mobile-first

Implementasi dari [ARCHITECTURE.md](./ARCHITECTURE.md): metaframework PWA **mobile-first**
di atas **Axum** (server) + **Leptos** (SSR + hydration Wasm) + **SQLite** (tertanam),
dengan *file-based routing* dan *Auto-PWA Generator* yang berjalan saat build.

## Struktur

```
.
├── ARCHITECTURE.md          # spesifikasi & visi
├── Cargo.toml               # workspace + profil wasm-release
├── rust-toolchain.toml      # stable + target wasm32-unknown-unknown
├── crates/
│   ├── sayagi-build/        # codegen: rute, config, manifest, service worker, ikon
│   ├── sayagi/              # runtime: MobileLayout, HeaderBar, BottomNav, param(), PWA wiring
│   └── sayagi-cli/          # CLI `sayagi`: new / dev / build / routes / pwa
└── app/                     # contoh aplikasi (mobile-pwa)
    ├── Sayagi.toml          # SATU sumber konfigurasi
    ├── build.rs             # memanggil generator pada setiap build
    ├── public/              # aset statis (manifest/sw/ikon ditulis generator)
    ├── style/main.css
    └── src/
        ├── main.rs          # binary Axum (SSR, server functions, WebSocket)
        ├── lib.rs           # entry `hydrate()` + include rute/config hasil generate
        ├── app.rs           # shell HTML + router
        ├── api.rs           # server functions (#[server])
        ├── db.rs            # SQLite WAL + migrasi + seed
        ├── service_worker.rs
        ├── components/      # komponen milik aplikasi
        └── pages/           # ← rute berasal dari sini
            ├── index.rs             → /
            ├── dashboard.rs         → /dashboard
            ├── offline.rs           → /offline
            └── containers/
                ├── index.rs         → /containers
                └── [id].rs          → /containers/:id
```

## Prasyarat

```bash
rustup toolchain install stable --profile minimal
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos --locked     # disediakan otomatis oleh `sayagi dev|build`
```

## Menjalankan contoh

```bash
cd app
cargo leptos watch          # http://127.0.0.1:3000 (hot reload)
cargo leptos build --release
```

Atau lewat CLI metaframework:

```bash
cargo run -p sayagi-cli -- routes --dir app   # tabel rute hasil pemindaian
cargo run -p sayagi-cli -- pwa --dir app      # regenerate manifest/sw/ikon
cargo run -p sayagi-cli -- new /tmp/demo-app  # scaffold proyek baru
```

## Bagaimana bagian-bagiannya saling terhubung

| Konsep di ARCHITECTURE.md | Wujudnya di repo ini |
| --- | --- |
| Page-Based Routing | [`sayagi-build/src/routes.rs`](./crates/sayagi-build/src/routes.rs) memindai `src/pages` → `$OUT_DIR/routes.rs` berisi `#[path] pub mod`, tabel `ROUTES`, dan komponen `FileRoutes` |
| Seamless Data Fetching | [`app/src/api.rs`](./app/src/api.rs) — fungsi `#[server]` dipublikasikan otomatis di `/api/<nama_fungsi><hash>` |
| Auto-PWA Generator | [`sayagi-build/src/pwa.rs`](./crates/sayagi-build/src/pwa.rs) menulis `manifest.webmanifest`, `sw.js`, dan `icons/*.png` dari `Sayagi.toml` |
| Embedded Service Worker Engine | Strategi cache digenerate dari `Sayagi.toml` ([template](./crates/sayagi-build/src/templates/sw.js)): network-first untuk `/api/`, SWR untuk `/pkg/`, navigasi punya fallback `/offline` |
| Mobile UI Shell Components | [`sayagi/src/components.rs`](./crates/sayagi/src/components.rs) — `MobileLayout`, `HeaderBar`, `BottomNav`, `PullToRefresh` |
| Embedded SQLite (WAL) | [`app/src/db.rs`](./app/src/db.rs) — pool sqlx, `journal_mode=WAL`, migrasi + seed otomatis |
| Async Runtime: Tokio | `#[tokio::main]` di [`app/src/main.rs`](./app/src/main.rs) |
| Build Tooling | [`sayagi-cli`](./crates/sayagi-cli/src/main.rs) membungkus `cargo leptos`, plus codegen sendiri |
| Mobile UI Shell (Gestures) | `PullToRefresh` memakai touch events; hanya aktif saat window berada di paling atas |

### Konvensi halaman

Setiap berkas di `src/pages` harus mengekspor satu komponen bernama `Page`:

```rust
use leptos::prelude::*;
use sayagi::{param, MobileLayout};

#[component]
pub fn Page() -> impl IntoView {
    let id = param("id");                      // dari segmen [id].rs
    view! {
        <MobileLayout title="Detail" show_back=true>
            <h2>"Container " {move || id.get()}</h2>
        </MobileLayout>
    }
}
```

| Nama berkas | URL |
| --- | --- |
| `index.rs` | `/` |
| `dashboard.rs` | `/dashboard` |
| `folders/index.rs` | `/folders` |
| `folders/[id].rs` | `/folders/:id` |
| `files/[...path].rs` | `/files/*path` |

Berkas yang diawali `_` dan `mod.rs` diabaikan, jadi partial bisa diletakkan
berdampingan dengan halaman.

## Catatan perilaku

* **Service worker hanya aktif di build release.** Di build debug, `cargo leptos
  watch` menyajikan `/pkg/*` tanpa hash nama berkas, sehingga cache
  *stale-while-revalidate* dapat menyajikan campuran JS/Wasm dari build lama dan
  merusak hot reload. Logikanya ada di
  [`app/src/service_worker.rs`](./app/src/service_worker.rs).
  Untuk menguji perilaku offline, jalankan build release.
* **Nama berkas halaman boleh mengandung `[...]`.** `[id].rs` dideklarasikan lewat
  `#[path = ".../[id].rs"] pub mod …` di `$OUT_DIR/routes.rs`, jadi tidak perlu
  nama modul yang berbeda dari nama rute.
* **`<A>` tidak menerima atribut tambahan** di leptos_router 0.8. Karena itu
  komponen tombol (`LinkButton`) menaruh gaya di elemen pembungkus dan CSS
  menyasar `.sy-btn > a`; status aktif navigasi bawah memakai
  `aria-current="page"` yang dipasang router sendiri.

## Deviasi kecil dari dokumen

* **Service worker** digenerate sebagai berkas JS klasik
  ([template](./crates/sayagi-build/src/templates/sw.js)), bukan dikompilasi ke Wasm —
  spesifikasi Web Platform mewajibkan service worker berupa worker JavaScript.
  Sisi Rust tetap yang memasang dan mengelola versinya
  ([`app/src/service_worker.rs`](./app/src/service_worker.rs)).
* **`sqlx::query_as!`** diganti `sqlx::query_as` / `sqlx::query` versi runtime agar
  build tidak membutuhkan `DATABASE_URL` saat kompilasi.
* Ikon aplikasi digenerate (PNG murni tanpa dependensi) sehingga proyek baru langsung
  bisa dipasang ke home screen tanpa menyertakan berkas biner di template.

## Verifikasi

```bash
# Unit test generator (rute, PNG, template) dan CLI
cargo test -p sayagi-build -p sayagi-cli

# Kedua sisi aplikasi
cargo check -p mobile-pwa --features ssr
cargo check -p mobile-pwa --target wasm32-unknown-unknown --no-default-features --features hydrate

# Build penuh: binary Axum + bundle Wasm + aset PWA
cargo leptos build --release --manifest-path app/Cargo.toml

# Jalankan
cargo leptos serve --release --manifest-path app/Cargo.toml
```

Saat server jalan, hal-hal ini bisa dicek langsung:

```bash
curl -s localhost:3000/containers | grep sy-card__title        # SSR + SQLite
curl -sI localhost:3000/manifest.webmanifest                    # manifest hasil generate
curl -sI localhost:3000/sw.js                                   # service worker hasil generate
curl -sI localhost:3000/icons/icon-192.png                      # ikon hasil generate
```

Endpoint WebSocket contoh (dari diagram arsitektur) tersedia di `ws://127.0.0.1:3000/api/ws`
dan menggemakan setiap pesan teks:

```bash
websocat ws://127.0.0.1:3000/api/ws   # kirim teks apa pun, dibalas {"echo":"…"}
```
