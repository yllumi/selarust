# Sayagi Rust — Arsitektur

Dokumen ini menjelaskan **arsitektur boilerplate** Sayagi Rust: lapisan sistem,
tanggung jawab tiap crate, kontrak saat build, alur runtime, konfigurasi, dan
titik ekstensi ketika dipakai untuk membangun aplikasi PWA mobile.

Cara menjalankan, catatan perilaku, dan hasil pengukuran ada di
[README.md](./README.md); instruksi untuk agen/AI ada di
[.github/copilot-instructions.md](./.github/copilot-instructions.md).

## 1. Ringkasan

Satu basis kode Rust, dikompilasi **dua kali**:

```text
app/src/**  ──(features=ssr)──►  binary Axum   : SSR, server functions, WebSocket, aset
            ──(features=hydrate, wasm32)──►  modul Wasm : hidrasi + interaktivitas browser
```

Tujuannya: kenyamanan ala Sayagi versi PHP (rute berbasis berkas, memanggil
fungsi server layaknya fungsi lokal) dengan type-safety, performa, dan efisiensi
memori Rust.

### Prinsip desain

| # | Prinsip | Konsekuensi |
| --- | --- | --- |
| 1 | **Satu sumber konfigurasi.** | `app/Sayagi.toml` menentukan rute, nilai manifest, service worker, ikon, alamat server, dan konstanta aplikasi. Tidak ada daftar rute atau aset PWA yang ditulis manual. |
| 2 | **Codegen saat build, bukan macro runtime.** | `sayagi-build` dipanggil `build.rs`; hasilnya berkas Rust/JS/PNG nyata yang bisa dibaca dan di-debug, dan nama berkas seperti `[id].rs` tetap bisa dipakai apa adanya. |
| 3 | **Framework dan aplikasi dipisah crate.** | Primitif generik di `crates/sayagi*`; kebijakan produk di `app`. Aplikasi boleh bergantung ke framework, tidak sebaliknya. |
| 4 | **Offline-first sebagai jalur utama.** | Semua mutasi lewat antrean lokal lebih dulu, online maupun offline; tidak ada jalur "khusus offline" yang terpisah. |
| 5 | **Satu tempat untuk satu aturan.** | Cara sebuah event mengubah daftar hanya ditulis di `TodoEvent::apply` (lihat §5.5), dipakai sinkronisasi realtime *dan* replay antrean. |
| 6 | **PWA tanpa ketergantungan jaringan pihak ketiga.** | Runtime Workbox di-vendor dan disajikan dari origin sendiri, jadi worker bisa dipasang saat perangkat offline. |

## 2. Peta lapisan

```text
┌─ Browser ────────────────────────────────────────────────────────────────────┐
│ 1. HTML hasil SSR ──► 2. Wasm ter-hidrasi                                    │
│ 3. UI shell      : MobileLayout, HeaderBar, BottomNav, BottomSheet, gesture  │
│ 4. State klien   : TodoStore = server ⊕ snapshot ⊕ antrean mutasi            │
│ 5. Realtime      : WebSocket /api/ws ──► apply(event) ──► store              │
│ 6. Service worker: Workbox — precache, runtime cache, fallback offline       │
└──────────────────────────────────────────────────────────────────────────────┘
        │ HTTP: HTML, /pkg/* (Wasm+CSS), /api/* (server functions), /api/ws (WS)
        ▼
┌─ Server (Axum, feature ssr) ─────────────────────────────────────────────────┐
│ Router      : /api/ws → handler WebSocket                                    │
│               leptos_routes_with_context → generate_route_list(App)          │
│               fallback → file_and_error_handler (aset statis + /pkg)         │
│ Context     : LeptosOptions + SqlitePool + SyncHub (disediakan via FromRef)  │
│ Middleware  : CompressionLayer → asset_cache_control (ETag + must-revalidate)│
└──────────────────────────────────────────────────────────────────────────────┘
        ▼
   SQLite tertanam (WAL) · aset PWA hasil generate di `app/public/`
```

| Lapisan | Teknologi | Tanggung jawab | Berkas kunci |
| --- | --- | --- | --- |
| Build-time | Rust biasa + `include_str!` | Memindai halaman, menulis rute/konstanta/manifest/SW/ikon | [`crates/sayagi-build/src/`](./crates/sayagi-build/src/) |
| Server | Axum + `leptos_axum` + Tokio | SSR, server functions, aset statis, WebSocket, cache header | [`app/src/main.rs`](./app/src/main.rs) |
| Router halaman | `leptos_router` | Pohon `<Routes>` hasil generate, transisi antar halaman | `$OUT_DIR/routes.rs` via [`app/src/lib.rs`](./app/src/lib.rs) |
| Runtime UI | Leptos (Wasm) | Komponen mobile shell, gestur, bottom sheet | [`crates/sayagi/src/components.rs`](./crates/sayagi/src/components.rs) |
| Data server | SQLite + `sqlx` | Skema, query, seed; hanya dibangun di sisi server | [`app/src/db.rs`](./app/src/db.rs) |
| Jalur data | `#[server]` server functions | Kontrak RPC Wasm ↔ Axum di bawah `/api/` | [`app/src/api.rs`](./app/src/api.rs) |
| State klien | Sinyal Leptos + IndexedDB | Snapshot lokal, antrean mutasi, overlay optimistis, replay | [`app/src/store.rs`](./app/src/store.rs) |
| Sinkronisasi | WebSocket + `broadcast` | Siaran mutasi ke semua klien, snapshot saat `subscribe` | [`app/src/sync.rs`](./app/src/sync.rs), [`app/src/realtime.rs`](./app/src/realtime.rs) |
| Offline & PWA | IndexedDB + Workbox 7 | Penyimpanan lokal, registrasi worker, status koneksi | [`crates/sayagi/src/offline.rs`](./crates/sayagi/src/offline.rs), [`crates/sayagi/src/pwa.rs`](./crates/sayagi/src/pwa.rs) |
| Styling | CSS tunggal | Tema, token gerak Material 3, seluruh aturan transisi | [`app/style/main.css`](./app/style/main.css) |

## 3. Peta crate

```text
sayagi-cli ──(scaffold, memanggil cargo leptos)──► sysroot
sayagi-build ──(build-dependency)──► app
sayagi ──(runtime)──► app
```

| Crate | Peran | Dikompilasi | Isi kunci |
| --- | --- | --- | --- |
| [`crates/sayagi-build`](./crates/sayagi-build/) | Generator saat build (dan untuk perintah `sayagi pwa`). Tanpa dependensi Leptos/Async supaya aman dipanggil dari `build.rs`. | Host (build script & CLI) | `routes.rs` (rute + `<Routes>`), `config.rs` (TOML → konstanta), `pwa.rs` (manifest, `sw.js`, Workbox), `png.rs` (ikon), `vendor/workbox/` |
| [`crates/sayagi`](./crates/sayagi/) | Primitif runtime yang tidak tahu produk: shell UI, gestur, arah transisi, penyimpanan offline, klien WebSocket, wiring PWA, `param()`. | Dua kali: `ssr` dan `hydrate` (wasm32) | `components.rs`, `transitions.rs`, `offline.rs`, `sync.rs`, `pwa.rs`, `params.rs` |
| [`crates/sayagi-cli`](./crates/sayagi-cli/) | CLI `sayagi`: `new`, `dev`, `build`, `routes`, `pwa`. Tipis di atas `cargo leptos`, tidak menyembunyikan toolchain. | Host | `src/main.rs`, `src/templates.rs` + `templates/app/**` |
| [`app`](./app/) (`mobile-pwa`) | Aplikasi contoh yang memakai semua kemampuan: halaman, server functions, tabel todo, store klien, hub realtime. Sekaligus cetakan untuk proyek baru. | Binary (`ssr`) + `cdylib`/`rlib` (`hydrate`) | `Sayagi.toml`, `build.rs`, `src/{main,lib,app,api,db,store,realtime,sync,service_worker}.rs`, `src/pages/`, `style/main.css` |

Aturan ketergantungan yang dijaga:

* `sayagi-build` **tidak** bergantung pada `leptos`/`tokio` — ia hanya menulis teks dan PNG.
* `sayagi` tidak mengetahui domain aplikasi (tidak ada tipe `Todo` di sana).
* `sayagi-cli` tidak mengompilasi apa pun sendiri; `dev`/`build` meneruskan ke `cargo leptos`.
* `app` adalah satu-satunya crate yang tahu produk.

## 4. Kontrak saat build

```text
app/Sayagi.toml        ┐
app/src/pages/**/*.rs  ├─► sayagi-build::generate() ─┬─► $OUT_DIR/routes.rs      → include! di app/src/lib.rs
app/public/            ┘   (dipanggil app/build.rs)  ├─► $OUT_DIR/config.rs      → include! di app/src/lib.rs
                                                     └─► app/public/{manifest.webmanifest, sw.js, icons/*.png, workbox/*}
```

| Artefak | Isi | Dipakai oleh |
| --- | --- | --- |
| `$OUT_DIR/routes.rs` | `#[path]` untuk setiap halaman, tabel `ROUTES: &[RouteInfo]`, komponen `NotFound` dan `FileRoutes` (`<Routes transition=…>`) | Router klien & `generate_route_list(App)` di server |
| `$OUT_DIR/config.rs` | Konstanta yang dipanggang dari TOML: `APP_NAME`, `APP_SHORT_NAME`, `LANG`, `THEME_COLOR`, `MANIFEST_PATH`, `SERVICE_WORKER_PATH`, `PAGE_TRANSITIONS`, `PRIMARY_ICON`, `DATABASE_URL`, … | `shell()` dan shell klien |
| `manifest.webmanifest` | Manifest PWA (nama, warna, ikon, `display`, `start_url`, `scope`) | Browser (pemasangan ke home screen) |
| `sw.js` | Service worker + strategi cache Workbox, versi cache dari sidik jari sumber aplikasi | Browser (registrasi dari `sayagi::register_service_worker`) |
| `icons/*.png` | Ikon 192/512 biasa dan *maskable*, digambar langsung oleh generator (tanpa dependensi gambar) | Manifest |
| `workbox/` | Runtime Workbox 7.4.1 yang di-vendor | `sw.js` |

Semua berkas hasil generate diawali header `@generated` dan diabaikan git
(lihat [.gitignore](./.gitignore)); menghapusnya aman karena dibuat ulang setiap
`cargo build` atau `sayagi pwa`.

### Aturan file-based routing

| Berkas di `src/pages` | URL | `RouteKind` |
| --- | --- | --- |
| `index.rs` | `/` | static |
| `dashboard.rs` | `/dashboard` | static |
| `todos/index.rs` | `/todos` | static |
| `todos/[id].rs` | `/todos/:id` | dynamic |
| `files/[...path].rs` | `/files/*path` | wildcard |

* Nama berkas `[param]` menjadi segmen dinamis, `[...rest]` menjadi catch-all.
* Berkas `mod.rs` dan yang diawali `_` diabaikan, sehingga partial bisa
  diletakkan berdampingan dengan halaman.
* Urutan pencocokan: **static → dynamic → wildcard**, supaya rute paling
  spesifik menang.
* Setiap modul halaman wajib mengekspor `#[component] pub fn Page() -> impl IntoView`.

### Catatan keputusan pada kontrak build

* **Build script, bukan proc macro.** Nama berkas `[id].rs` bukan identifier
  Rust yang sah; dengan codegen, berkas itu dideklarasikan lewat
  `#[path = "…/[id].rs"] pub mod …`. Bonusnya: pemindaian rute bisa dijalankan
  tanpa membangun aplikasi, seperti yang dilakukan `sayagi routes`.
* **`hash-files` sengaja mati** pada metadata Leptos (lihat
  [`app/Cargo.toml`](./app/Cargo.toml)): dengan hashing, rebuild CSS inkremental
  saat `cargo leptos watch` tidak memperbarui nama berkas sehingga HTML menunjuk
  CSS basi. Kesegaran ditangani ETag + `must-revalidate` untuk `/pkg` (§5.6).

## 5. Alur runtime

### 5.1 Permintaan pertama (SSR) dan hidrasi

1. Axum menerima `GET /todos`; `leptos_routes_with_context` memilih komponen
   halaman dari `FileRoutes`.
2. Selama render, `Resource` di halaman memanggil server function (`list_todos`)
   langsung sebagai fungsi Rust — tidak ada HTTP bolak-balik saat SSR.
3. Server mengirim HTML lengkap (shell dari `app.rs` + isi halaman) beserta tag
   PWA (manifest, `theme-color`, `apple-touch-icon`).
4. Browser memuat bundle `/pkg/mobile-pwa.{js,wasm}` dan menjalankan `hydrate()`
   ([`app/src/lib.rs`](./app/src/lib.rs)): memasang service worker (release saja),
   memulai pemantauan koneksi, lalu `hydrate_body(App)`.

### 5.2 Navigasi klien dan transisi halaman

* Klik `<A>`/`LinkButton` ditangani `leptos_router` di klien: hanya pohon rute
  yang ditukar, dokumen tidak dimuat ulang.
* `MobileLayout` melaporkan **kedalaman rute** lewat `sayagi::note_route_depth`
  dari dalam update callback view transition, sehingga kelas arah
  (`sy-nav-child` / `sy-nav-parent` / `sy-nav-level`) sudah terpasang saat
  browser menyusun lapisan animasi. Riwayat browser tidak dipakai karena tidak
  bisa membedakan "mundur" dari "pindah selevel".
* Arah animasi ditentukan kedalaman, bukan riwayat: turun = geser kiri, naik =
  geser kanan, selevel = fade. Angka geraknya token Material 3 di stylesheet.
* Chrome shell (header, banner offline, navigasi bawah) punya
  `view-transition-name` sendiri yang **tidak** dianimasikan, supaya tidak ikut
  berkedip saat isi halaman bergeser. Detail dan pengukurannya di
  [README](./README.md#mobile-shell).

### 5.3 Jalur data

Semua komunikasi data internal memakai server functions `#[server(Nama, "/api")]`
di [`app/src/api.rs`](./app/src/api.rs) — tanpa menulis endpoint REST manual.

| Sifat | Bentuk | Alasan |
| --- | --- | --- |
| Baca (`list_todos`, `get_todo`, `get_todo_stats`) | `input = GetUrl` → HTTP GET | Service worker hanya bisa meng-cache GET, jadi daftar tetap terbaca saat offline |
| Tulis (`create_todo`, `update_todo`, `set_todo_completed`, `delete_todo`) | POST (bawaan) | Mutasi tidak boleh disajikan dari cache |
| Nama rute | `<nama_fungsi><hash lokasi>` | Karena itu `Sayagi.toml` menaruh prefix `/api/` di `network_first`, bukan daftar URL yang harus diperbarui tiap ada fungsi baru |

Server function mengambil pool dari context Leptos (`crate::db::pool()`), menulis
ke SQLite, lalu menyiarkan perubahan ke hub realtime (`publish`). Validasi
(mis. `MAX_TITLE`) hidup di sisi server saja dan tidak ikut dikompilasi ke Wasm.

### 5.4 Offline-first (state klien)

`TodoStore` ([`app/src/store.rs`](./app/src/store.rs)) menggabungkan tiga sumber:

```text
server   : daftar terakhir dari server (SSR / refetch / snapshot WebSocket)
snapshot : salinan di IndexedDB (kunci "todos"), dipakai saat server tak terjangkau
pending  : mutasi yang belum tersinkron, diterapkan sebagai overlay optimistis

items() = overlay(server ∪ snapshot, pending)
```

| Tahap | Yang terjadi | Detail penting |
| --- | --- | --- |
| Muat | `load()` (snapshot IndexedDB) → tampilkan daftar lokal lebih dulu, lalu `refresh()` dari server | Daftar terlihat seketika walau jaringan lambat/putus |
| Mutasi | `enqueue(TodoMutation)` → tulis ke IndexedDB `queue` → terapkan optimistis | Baris baru memakai **id sementara negatif** |
| Koalesensi | `coalesce()` membuang mutasi yang sudah tercakup mutasi berikutnya | `Create` tidak digabung karena id server belum diketahui |
| Replay | `flush()` mengirim antrean berurutan, berhenti pada kegagalan pertama | Id sementara dipetakan ke id server (`translate` + `remap`) sebelum sisa antrean dikirim |
| Ulang | Antrean dibaca ulang dari IndexedDB (bukan dari memori) setelah replay | Maksimum `FLUSH_ROUNDS` putaran supaya mutasi baru ikut terkirim |

Fungsi murni (`coalesce`, `translate`, `overlay`) diuji unit tanpa browser;
bagian yang menyentuh IndexedDB dan server functions hanya membungkusnya.

### 5.5 Sinkronisasi realtime

```text
klien → server : {"type":"subscribe"}            → {"type":"snapshot","todos":[…]}   (saat kanal terbuka)
                 {"type":"ping"}                 → {"type":"pong"}
                 teks apa pun                    → {"type":"echo", …}
server → klien : {"type":"todo","event":{…}}     (setiap mutasi, ke semua klien)
```

* Server: [`app/src/sync.rs`](./app/src/sync.rs) — `SyncHub` (kanal `broadcast`
  berbuffer 128) menerima event dari server function dan meneruskannya ke setiap
  socket; `handle_socket` mengirim `hello`, menangani `subscribe`, dan menjawab
  ping.
* Klien: [`crates/sayagi/src/sync.rs`](./crates/sayagi/src/sync.rs) — koneksi
  dengan backoff eksponensial 0,5 s → 30 s, ping 25 s; [`app/src/realtime.rs`](./app/src/realtime.rs)
  menerjemahkan pesan menjadi aksi store (`snapshot` → `replace`, `todo` → `apply`).
* **Satu penulis aturan:** `TodoEvent::apply` di `api.rs` adalah satu-satunya
  tempat yang tahu cara sebuah event mengubah daftar. Sinkronisasi realtime dan
  replay antrean offline memakai fungsi yang sama, jadi tidak ada dua
  implementasi yang bisa berbeda.

### 5.6 Lapisan cache PWA

| Lapisan | Cakupan | Strategi | Alasan |
| --- | --- | --- | --- |
| Precache berversi | `precache` di `Sayagi.toml` (`/`, `/offline`, manifest) + bundle shell `/pkg/*` + dokumen rute yang pernah dibuka | Workbox `precacheAndRoute` | Shell harus siap dipakai tanpa jaringan; revisi berasal dari sidik jari sumber, jadi build baru selalu mengunduh ulang |
| Runtime: `/api/` | Server functions | network-first, fallback cache | Data harus segar saat online, tetap terbaca saat offline |
| Runtime: `/pkg/` | Wasm/JS/CSS | network-first, fallback cache | Nama berkas tidak di-hash, jadi stale-while-revalidate berisiko mencampur wasm lama dengan HTML baru |
| Runtime: `/icons/`, `/assets/` | Aset statis | stale-while-revalidate | Jarang berubah, aman disajikan dari cache lebih dulu |
| Fallback navigasi | `offline_fallback` (`/offline`) | `setCatchHandler` | Permintaan dokumen yang gagal tetap mendapat halaman bermakna |
| Batas cache | `runtime_max_entries`, `runtime_max_age_seconds` | `ExpirationPlugin` | Mencegah cache tumbuh tanpa batas |

Sisi server melengkapinya lewat `asset_cache_control`
([`app/src/main.rs`](./app/src/main.rs)): `/pkg/*` dikirim dengan
`max-age=0, must-revalidate` + ETag yang dihitung dari metadata berkas
(termasuk `content-encoding`, karena responsnya `Vary: accept-encoding`).
Hasilnya: deploy langsung terpakai, tetapi muat ulang berikutnya hanya
`304 Not Modified` sehingga bundle ~600 KB tidak diunduh ulang.

Pada build debug service worker **tidak** dipasang; worker/cache dari build
release sebelumnya justru dibersihkan supaya HTML basi tidak merusak hot reload.

### 5.7 Invarian lintas lapisan

Aturan berikut yang membuat lapisan-lapisan di atas tetap konsisten:

1. Mutasi data dari UI selalu lewat `TodoStore` (antrean), tidak pernah memanggil
   server function tulis secara langsung.
2. Setiap perubahan data yang perlu diketahui klien lain disiarkan dari server
   function; tipe event ditambah di satu tempat (`TodoEvent`) beserta varian
   `apply`.
3. Server function baca memakai `input = GetUrl` agar bisa di-cache service
   worker; fungsi tulis tidak pernah di-cache.
4. Berkas yang ditandai `@generated` tidak diedit manual; ubah generator atau
   `Sayagi.toml`.
5. Halaman berada di dalam `MobileLayout` agar kedalaman rute (arah transisi)
   dan chrome shell konsisten. Halaman di luarnya hanya mendapat fade bawaan.

## 6. Konfigurasi: `Sayagi.toml`

Satu berkas ini mengendalikan rute, manifest, service worker, ikon, database,
dan konstanta yang dipanggang ke aplikasi. Semua bagian selain `[app]` boleh
dihilangkan (memakai bawaan).

| Bagian | Kunci | Bawaan | Efek |
| --- | --- | --- | --- |
| `[app]` | `name`, `short_name` | — (wajib) | Nama aplikasi; `APP_NAME`/`APP_SHORT_NAME` |
| | `description` | `""` | `meta description` + manifest |
| | `lang` | `"en"` | `<html lang>` |
| | `theme_color`, `background_color` | `"#0f172a"` | `meta theme-color` + manifest |
| | `start_url`, `scope` | `"/"` | Manifest PWA |
| | `display` | `"standalone"` | Manifest PWA |
| | `orientation` | `"portrait"` | Manifest PWA |
| | `site_addr` | `"127.0.0.1:3000"` | Alamat dev server (dipakai `cargo leptos`) |
| | `page_transitions` | `true` | `PAGE_TRANSITIONS` → atribut `transition` pada `<Routes>` |
| | `assets_dir` | `"public"` | Folder tempat manifest, `sw.js`, ikon, dan Workbox ditulis |
| `[routes]` | `dir` | `"src/pages"` | Akar pemindaian rute |
| `[pwa]` | `service_worker` | `true` | `SERVICE_WORKER_ENABLED`; kalau `false`, `sw.js` lama dihapus |
| | `offline_fallback` | `"/offline"` | Halaman fallback navigasi |
| | `icons` | `[192, 512]` | Ukuran ikon yang digambar |
| | `precache` | `["/", "/offline", "/manifest.webmanifest"]` | Entri precache berversi |
| | `network_first` | `["/api/"]` | Prefix yang memakai jaringan lebih dulu |
| | `stale_while_revalidate` | `["/pkg/", "/icons/", "/assets/"]` | Prefix yang memakai cache dulu |
| | `runtime_max_entries`, `runtime_max_age_seconds` | `128`, 30 hari | Batas cache runtime (Workbox `ExpirationPlugin`) |
| `[database]` | `url` | `"sqlite://sayagi.db"` | Path relatif diresolusi terhadap direktori biner; bisa ditimpa env `SAYAGI_DATABASE_URL` |
| | `max_connections` | `5` | Ukuran pool |
| | `seed` | `true` | Isi data contoh saat tabel masih kosong |

## 7. Konvensi & titik ekstensi

| Ingin menambah/mengubah | Sentuh | Catatan |
| --- | --- | --- |
| Halaman baru | Berkas di `app/src/pages/**` yang mengekspor `Page` | Rute muncul otomatis pada build berikutnya |
| Rute dinamis | Berkas `[param].rs`, ambil nilainya dengan `sayagi::param("param")` | Urutan pencocokan mengikuti §4 |
| Aksi data baru | Server function `#[server(Nama, "/api")]` di `app/src/api.rs` (+ query di `db.rs`) | Baca pakai `input = GetUrl`; kalau klien lain perlu tahu, siarkan event |
| Jenis event sinkronisasi baru | Varian `TodoEvent` + cabang `apply` di `app/src/api.rs` | Satu-satunya penulis aturan (§5.5) |
| State klien baru | Pola `TodoStore`: sinyal + penyimpanan lokal + antrean, di-`provide_context` dari `App` | Ikuti invarian §5.7 |
| Komponen UI | Primitif generik → `crates/sayagi/src/components.rs`; khusus aplikasi → `app/src/components/` | Kelas selalu berprefix `sy-`; gaya hanya di `app/style/main.css` |
| Entri navigasi bawah | `provide_context(NavItems::new([...]))` di `app/src/app.rs` | Halaman bisa menimpanya lewat prop `nav` pada `MobileLayout` |
| Tema/warna | Variabel `:root` di `app/style/main.css` | Komponen tidak membawa warna sendiri |
| Kecepatan & gaya animasi | Token `--sy-motion-*` di `app/style/main.css` | Angka mengikuti Material 3; `--sy-motion-travel`, durasi, easing |
| Nama `view-transition-name` baru | Stylesheet + (kalau perlu) `crates/sayagi/src/transitions.rs` | Chrome shell (`sy-chrome-*`) harus tetap `animation: none` |
| Template proyek baru (`sayagi new`) | `crates/sayagi-cli/templates/app/**` | Berkas `.tmpl` dirender dengan `project_name`, `crate_name`, `project_title`, `sayagi_root`, `site_addr`; **jaga tetap sinkron dengan `app/`** |
| Skema database | `migrate()` di `app/src/db.rs` | Idempoten (`CREATE TABLE IF NOT EXISTS`), dijalankan setiap boot |
| Runtime Workbox | Prosedur di [`crates/sayagi-build/vendor/workbox/NOTICE.md`](./crates/sayagi-build/vendor/workbox/NOTICE.md) | Jangan sunting berkas vendored langsung |

## 8. Keputusan arsitektur

| Keputusan | Alasan | Konsekuensi |
| --- | --- | --- |
| SSR + hidrasi dari satu sumber Rust | Satu bahasa untuk server dan klien, tidak ada duplikasi model di dua bahasa | Dua target build (`ssr`, `hydrate`) dan disiplin `#[cfg]` pada kode yang menyentuh server/browser |
| Codegen lewat `build.rs`, bukan proc macro | Nama berkas seperti `[id].rs` bisa dipakai apa adanya; hasilnya bisa dibaca & diuji | Perubahan yang tidak menyentuh `src/**` perlu `cargo build` ulang untuk regenerate |
| Workbox di-vendor, bukan CDN/npm saat runtime | Worker bisa dipasang saat perangkat offline; tidak ada origin pihak ketiga | Berkas runtime ikut di repo dan harus diperbarui manual (ada prosedurnya di NOTICE) |
| Service worker mati di build debug | Aset dev disajikan ulang setiap rebuild; cache akan menyajikan HTML basi | Perilaku offline hanya bisa diuji pada build release |
| `/pkg/` network-first + ETag `must-revalidate` | Nama berkas bundle tidak di-hash, jadi cache panjang justru menahan deploy | Setiap muat ulang melakukan revalidasi (jawabannya `304`), bukan mengunduh ulang 600 KB |
| Antrean mutasi + overlay optimistis | Satu jalur kode untuk online dan offline; UI selalu responsif | Butuh id sementara negatif, koalesensi, dan remap id saat replay |
| Sinkronisasi via WebSocket, bukan polling/SSE | Dua tab/perangkat tetap sinkron tanpa permintaan periodik | Server menyimpan hub `broadcast`; klien menangani reconnect dengan backoff |
| Arah transisi dari kedalaman rute | Riwayat browser tidak membedakan "mundur" dari "pindah selevel" | Perlu laporan kedalaman dari `MobileLayout` di dalam update callback view transition |
| Chrome shell sebagai lapisan view transition statis | Chrome tidak boleh ikut gelap/terang saat isi halaman bergeser | Setiap elemen chrome wajib punya `view-transition-name` sendiri |
| `sayagi` tanpa domain aplikasi | Primitif bisa dipakai ulang proyek lain | Aplikasi menulis lapisan kebijakannya sendiri (`store.rs`, `api.rs`) |

## 9. Batas yang disengaja (belum ada di boilerplate)

* **Tanpa autentikasi/multi-pengguna.** Contoh ini satu pengguna dengan SQLite
  lokal; hub realtime menyiarkan semua mutasi ke semua klien.
* **Tanpa alat migrasi bertingkat.** Skema dibuat `IF NOT EXISTS` saat boot;
  perubahan kolom yang merusak perlu penanganan manual.
* **Antrean offline bersifat per-fitur.** `TodoStore` + `TodoMutation` adalah
  contoh pola; belum ada abstraksi generik (mis. ORM atau sinkronisasi otomatis).
* **Tanpa i18n, tanpa tema ganda, tanpa CI.** Bahasa antarmuka dan palet
  di-hardcode di contoh (CSS `:root` + teks halaman).
* **Verifikasi browser bersifat skrip eksternal.** Unit test Rust ada di repo;
  skenario gestur/offline/WebSocket dijalankan manual dengan Playwright terhadap
  build release (lihat [README](./README.md#verifikasi)).

## 10. Kemampuan bawaan dan wujudnya

| Kemampuan | Wujudnya |
| --- | --- |
| File-based routing | [`crates/sayagi-build/src/routes.rs`](./crates/sayagi-build/src/routes.rs) menghasilkan `ROUTES`, `FileRoutes`, `NotFound` |
| Auto-PWA (manifest, service worker, ikon) | [`crates/sayagi-build/src/pwa.rs`](./crates/sayagi-build/src/pwa.rs), [`png.rs`](./crates/sayagi-build/src/png.rs), template `sw.js` + Workbox vendored |
| Server functions | `#[server(Nama, "/api")]` di [`app/src/api.rs`](./app/src/api.rs), prefix `/api/` tercakup `network_first` |
| Mobile UI shell | [`crates/sayagi/src/components.rs`](./crates/sayagi/src/components.rs): `MobileLayout`, `BottomSheet`, `PullToRefresh`, swipe-back |
| Transisi halaman Material 3 | [`crates/sayagi/src/transitions.rs`](./crates/sayagi/src/transitions.rs) + blok transisi di [`app/style/main.css`](./app/style/main.css) |
| Offline storage | [`crates/sayagi/src/offline.rs`](./crates/sayagi/src/offline.rs) (IndexedDB) + [`app/src/store.rs`](./app/src/store.rs) (snapshot, antrean, replay) |
| Sinkronisasi realtime | [`crates/sayagi/src/sync.rs`](./crates/sayagi/src/sync.rs) + [`app/src/sync.rs`](./app/src/sync.rs) + [`app/src/realtime.rs`](./app/src/realtime.rs) |
| SQLite tertanam | [`app/src/db.rs`](./app/src/db.rs): WAL, pool, skema idempoten, seed |
| CLI & scaffold | [`crates/sayagi-cli`](./crates/sayagi-cli/) (`new`, `dev`, `build`, `routes`, `pwa`) |

## 11. Rujukan cepat

| Berkas | Untuk apa |
| --- | --- |
| [README.md](./README.md) | Menjalankan contoh, konvensi halaman, catatan perilaku, hasil pengukuran, verifikasi |
| [app/Sayagi.toml](./app/Sayagi.toml) | Konfigurasi tunggal aplikasi |
| [`crates/sayagi/src/`](./crates/sayagi/src/) | Primitif runtime (shell, transisi, offline, sync, PWA) |
| [`crates/sayagi-build/src/`](./crates/sayagi-build/src/) | Generator rute, konfigurasi, manifest, service worker, ikon |
| [`crates/sayagi-cli/templates/app/`](./crates/sayagi-cli/templates/app/) | Cetakan proyek baru (wajib sinkron dengan `app/`) |
| [.github/copilot-instructions.md](./.github/copilot-instructions.md) | Aturan kerja untuk agen/AI di repo ini |
