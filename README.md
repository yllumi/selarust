# Sayagi Rust — boilerplate PWA mobile-first

Metaframework PWA **mobile-first** di atas **Axum** (server) + **Leptos**
(SSR + hydration Wasm) + **SQLite** (tertanam), dengan *file-based routing* dan
*Auto-PWA Generator* yang berjalan saat build. Arsitekturnya — lapisan, kontrak
saat build, alur runtime — dijelaskan di [ARCHITECTURE.md](./ARCHITECTURE.md);
aturan kerja untuk agen/AI ada di
[.github/copilot-instructions.md](./.github/copilot-instructions.md).

## Struktur

```
.
├── ARCHITECTURE.md          # arsitektur: lapisan, kontrak build, alur runtime
├── .github/
│   └── copilot-instructions.md  # aturan kerja untuk agen/AI
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
        ├── api.rs           # server functions (#[server]) + event sinkronisasi
        ├── db.rs            # SQLite WAL + migrasi + seed
        ├── sync.rs          # hub broadcast + protokol /api/ws (server)
        ├── store.rs         # state klien: snapshot, antrean mutasi, replay
        ├── realtime.rs      # klien WebSocket → store
        ├── service_worker.rs
        ├── components/      # komponen milik aplikasi (+ SyncStrip)
        └── pages/           # ← rute berasal dari sini
            ├── index.rs             → /
            ├── dashboard.rs         → /dashboard
            ├── offline.rs           → /offline  (precache + salinan lokal)
            └── todos/
                ├── index.rs         → /todos      (daftar, filter, bottom sheet)
                └── [id].rs          → /todos/:id  (ubah + hapus)
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

| Konsep ([ARCHITECTURE.md](./ARCHITECTURE.md)) | Wujudnya di repo ini |
| --- | --- |
| File-based routing (§4) | [`sayagi-build/src/routes.rs`](./crates/sayagi-build/src/routes.rs) memindai `src/pages` → `$OUT_DIR/routes.rs` berisi `#[path] pub mod`, tabel `ROUTES`, dan komponen `FileRoutes` |
| Jalur data: server functions (§5.3) | [`app/src/api.rs`](./app/src/api.rs) — fungsi `#[server]` dipublikasikan otomatis di `/api/<nama_fungsi><hash>`; baca lewat GET (`input = GetUrl`) supaya bisa di-cache |
| Auto-PWA: manifest, service worker, ikon (§4) | [`sayagi-build/src/pwa.rs`](./crates/sayagi-build/src/pwa.rs) menulis `manifest.webmanifest`, `sw.js`, runtime Workbox, dan `icons/*.png` dari `Sayagi.toml` |
| Lapisan cache PWA (§5.6) | `sw.js` memakai **Workbox 7** yang di-vendor lokal ([template](./crates/sayagi-build/src/templates/sw.js), [vendor + NOTICE](./crates/sayagi-build/vendor/workbox/NOTICE.md)): precache berversi, network-first untuk `/api/` dan `/pkg/`, SWR untuk ikon/aset, `setCatchHandler` menyajikan `/offline` |
| CRUD contoh | Server functions di [`app/src/api.rs`](./app/src/api.rs) + tabel `todos` di [`app/src/db.rs`](./app/src/db.rs): list, detail, tambah, ubah, tandai selesai, hapus |
| Offline-first: snapshot + antrean mutasi (§5.4) | [`crates/sayagi/src/offline.rs`](./crates/sayagi/src/offline.rs) (IndexedDB) + [`app/src/store.rs`](./app/src/store.rs) (overlay optimistis, koalesensi, replay) |
| Sinkronisasi realtime (§5.5) | [`app/src/sync.rs`](./app/src/sync.rs) (hub broadcast + `/api/ws`) dan [`app/src/realtime.rs`](./app/src/realtime.rs) (klien → store) |
| Mobile UI shell (§2, §5.2) | [`sayagi/src/components.rs`](./crates/sayagi/src/components.rs) — `MobileLayout`, `HeaderBar`, `BottomNav`, `PullToRefresh`, `BottomSheet`, swipe-back |
| SQLite tertanam (WAL) (§2) | [`app/src/db.rs`](./app/src/db.rs) — pool sqlx, `journal_mode=WAL`, migrasi + seed otomatis |
| Async Runtime: Tokio (§2) | `#[tokio::main]` di [`app/src/main.rs`](./app/src/main.rs) |
| Build tooling & CLI (§3) | [`sayagi-cli`](./crates/sayagi-cli/src/main.rs) membungkus `cargo leptos`, plus codegen sendiri ([`sayagi-build`](./crates/sayagi-build/src/lib.rs)) |

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
            <h2>"Todo " {move || id.get()}</h2>
        </MobileLayout>
    }
}
```

| Nama berkas | URL |
| --- | --- |
| `index.rs` | `/` |
| `dashboard.rs` | `/dashboard` |
| `todos/index.rs` | `/todos` |
| `todos/[id].rs` | `/todos/:id` |
| `files/[...path].rs` | `/files/*path` |

Berkas yang diawali `_` dan `mod.rs` diabaikan, jadi partial bisa diletakkan
berdampingan dengan halaman.

`build.rs` memindai ulang seluruh pohon `src/pages` pada setiap build (termasuk
subfolder baru), jadi menambah atau menghapus berkas halaman langsung terlihat
di tabel rute tanpa langkah manual.

## Mobile shell

Semua primitif hidup di crate `sayagi`, jadi aplikasi berikutnya tinggal pakai:

| Primitif | Kegunaan |
| --- | --- |
| `MobileLayout` | Shell halaman: header (judul, tombol kembali opsional, slot `actions`), area utama, navigasi bawah, banner offline, dan gestur swipe-back dari tepi kiri. |
| `BottomSheet` | Sheet dua arah (`RwSignal<bool>`): drag ke bawah melewati 28% tinggi panel, tap backdrop, atau `Escape` untuk menutup; scroll latar dikunci, fokus masuk ke panel lalu kembali ke elemen pemicu. |
| `PullToRefresh` | Tarik ke bawah untuk memuat ulang; dipakai halaman daftar. |
| `go_back()` | `history.back()` yang dipakai tombol kembali dan swipe-back. |

Catatan penerapan:

* **Swipe-back** hanya aktif di halaman dengan tombol kembali. Gestur baru
  "mengunci" setelah gerakan horizontal dominan (`dx > 12 px` dan `dx > 1.5 × dy`),
  jadi tidak berebut dengan scroll vertikal maupun pull-to-refresh. Ambang
  berpindah halaman `SWIPE_BACK_TRIGGER = 72 px`; sebelum itu halaman hanya
  bergeser sebagai umpan balik. Gestur bersifat idempoten supaya `touchend` yang
  disusul `touchcancel` tidak memanggil `history.back()` dua kali.
* **Transisi halaman** memakai View Transitions API lewat `<Routes transition=…>`
  (leptos_router), dengan nama transisi `sy-page` pada `.sy-main`. Arahnya
  ditentukan **kedalaman rute**, bukan arah riwayat: riwayat hanya tahu
  maju/mundur, sehingga pindah ke halaman selevel ikut terlihat seperti "maju"
  dan setiap mundur terlihat seperti "naik".

  | Perpindahan | Kelas di `<html>` | Animasi |
  | --- | --- | --- |
  | Masuk anak halaman (`/todos` → `/todos/:id`) | `sy-nav-child` | lama keluar ke kiri, baru masuk dari kanan |
  | Naik ke induk (`/todos/:id` → `/todos`, atau ke `/`) | `sy-nav-parent` | lama keluar ke kanan, baru masuk dari kiri |
  | Selevel (`/todos` → `/dashboard`) | `sy-nav-level` | silang fade |

  `MobileLayout` melaporkan kedalaman halaman lewat
  [`sayagi::note_route_depth`](./crates/sayagi/src/transitions.rs), dan komponen
  itu dibangun ulang **di dalam update callback view transition** — jadi kelasnya
  sudah ada sebelum browser menyusun lapisan animasi. Membaca URL di awal
  transisi tidak bisa dipakai: router baru `pushState` setelah transisi dimulai.
  Aturan kedalaman sendiri ada di Rust (`route_depth`) supaya bisa diuji.
  Dua shell bisa sempat hidup bersamaan saat rute diganti, jadi laporan berulang
  untuk **path yang sama** diabaikan — kalau tidak, kelas arah yang baru saja
  diputuskan akan ditimpa menjadi "selevel" (fade).
  Halaman di luar `MobileLayout` (kedalaman tidak dilaporkan) memakai aturan
  bawaan: fade saja.
* **Gerak mengikuti token Material 3.** Angkanya diambil dari
  `FadeForwardsPageTransitionsBuilder` (transisi halaman bawaan Android 14+/16 di
  Flutter): durasi `450ms`, jarak geser `25%` lebar layar, easing *emphasized
  decelerate* `cubic-bezier(0.05, 0.7, 0.1, 1)` untuk halaman masuk dan
  *emphasized accelerate* `cubic-bezier(0.3, 0, 0.8, 0.15)` untuk halaman keluar.
  Semuanya lewat custom property, jadi satu tempat cukup untuk menyetel ulang:

  | Token | Nilai | Dipakai untuk |
  | --- | --- | --- |
  | `--sy-motion-duration` | `450ms` | geser penuh kedua arah |
  | `--sy-motion-travel` | `25%` | jarak geser halaman |
  | `--sy-motion-duration-fade` | `300ms` | *fade through* halaman selevel |
  | `--sy-motion-ease-enter` | `cubic-bezier(0.05, 0.7, 0.1, 1)` | halaman masuk |
  | `--sy-motion-ease-exit` | `cubic-bezier(0.3, 0, 0.8, 0.15)` | halaman keluar |

  Setiap lapisan menjalankan **dua** animasi sekaligus: geser penuh 450 ms plus
  fade linear yang lebih pendek. Halaman masuk menyentuh opasitas 1 pada 75%
  durasi (337,5 ms) dan halaman keluar sudah hilang pada 25% durasi (112,5 ms),
  jadi isi halaman baru muncul perlahan di tengah geseran sementara halaman lama
  cepat menghilang. `.sy-main` juga diberi
  `background: var(--sy-bg)` supaya lapisan view transition tidak transparan
  (tanpa itu halaman lama terlihat menembus halaman baru saat fade).
  Pindah selevel memakai *fade through* 300 ms dengan pembagian 30/70.
* **Chrome shell tidak ikut beranimasi.** Header, banner offline, dan navigasi
  bawah adalah saudara `<main>`, jadi tanpa `view-transition-name` sendiri
  ketiganya jatuh ke snapshot `root` dan ikut cross-fade bawaan browser. Lapisan
  view transition juga disusun menurut urutan dokumen, bukan `z-index`: lapisan
  isi halaman (`sy-page`) yang bergeser tergambar **di atas** nav yang
  `position: fixed`, sehingga nav sempat berubah warna di tengah animasi —
  itulah kedipan yang terlihat pada setiap navigasi. Kini masing-masing punya
  lapisan sendiri (`sy-chrome-header`, `sy-chrome-banner`, `sy-chrome-nav`)
  dengan `animation: none`: snapshot lama disembunyikan dan snapshot baru tampil
  utuh sejak frame pertama, jadi chrome diam sementara isi halaman bergerak.
  Terukur di Chromium: warna area nav dan header sama persis sebelum, selama
  (dibekukan di 0/40/120/225/340/440 ms), dan sesudah transisi untuk ketiga arah
  navigasi.
* **Browser tanpa View Transitions API** tetap mendapat arah yang sama: kelas
  `sy-no-vt` dipasang saat API-nya tidak ada, dan CSS menganimasikan halaman baru
  (`sy-page-in-right` / `sy-page-in-left` / fade). Halaman lama tidak bisa
  dianimasikan pada jalur ini karena sudah dilepas router.
* **`prefers-reduced-motion: reduce` membuat semuanya fade.** Seluruh animasi
  transisi isi halaman dibungkus `@media (prefers-reduced-motion: no-preference)`,
  jadi lapisan `sy-page` memakai crossfade bawaan view transition (chrome shell
  tetap diam karena aturan `sy-chrome-*` berada di luar media query). Cek dengan
  `matchMedia('(prefers-reduced-motion: reduce)').matches` di DevTools; kalau
  hasilnya `true`, fade itu memang disengaja (pengaturan sistem).
* **`BottomSheet` di-teleport ke `document.body`.** `view-transition-name` pada
  `.sy-main` membuat *stacking context*, sehingga sheet `position: fixed` yang
  bersarang di dalamnya akan tergambar **di bawah** header dan navigasi bawah.
  Karena itu `BottomSheet` merender markupnya lewat `Portal`.
* **`overscroll-behavior-x: none`** dipasang di `html, body` pada stylesheet
  aplikasi: di Chrome Android, swipe dari tepi kiri adalah navigasi mundur bawaan
  yang bertabrakan dengan swipe-back shell (satu sapuan memicu dua kali mundur).
  Di browser tanpa gestur bawaan, properti itu tidak berpengaruh.
* **`scrollbar-gutter: stable` + `overflow-x: clip`** menemani properti di atas:
  saat scrollbar klasik (desktop) muncul atau hilang, lebar viewport berubah dan
  Chromium membatalkan view transition di tengah jalan — halaman tetap berpindah,
  tapi tanpa animasi. Ruang scrollbar selalu disediakan, dan geseran horizontal
  saat swipe-back dipotong dengan `clip` supaya tidak memunculkan scrollbar
  horizontal. `clip` (bukan `hidden`) dipakai agar `body` tidak menjadi scroll
  container dan header sticky tetap menempel.

## Offline & sinkronisasi

Data todo tidak lagi menunggu jaringan:

```
Resource SSR ─┐
              ├─► TodoStore.items()  = overlay(snapshot/server, antrean mutasi)
IndexedDB ────┘        │
                       └─► mutasi UI ─► antrean IndexedDB ─► replay ke server
                                                                  ▲
                              WebSocket /api/ws ─► event ─────────┘
```

* **Snapshot lokal.** Setiap daftar hasil server disimpan ke IndexedDB
  (`sayagi-offline` → store `kv`, kunci `todos`), jadi reload saat offline tetap
  menampilkan data terakhir.
* **Antrean mutasi.** `store.enqueue()` menulis ke store `queue` (autoIncrement)
  lalu menerapkan hasilnya secara optimistis: baris baru memakai id sementara
  negatif dan langsung tampil. `coalesce()` membuang mutasi yang sudah tercakup
  mutasi berikutnya (dua `Update` berurutan, `Complete` lalu `Delete`), tetapi
  tidak menggabungkan `Create` karena id server-nya baru diketahui saat replay.
* **Replay.** Saat koneksi kembali (efek pada status socket, `online`, atau tombol
  *Sinkronkan*), antrean dikirim berurutan; berhenti pada kegagalan pertama supaya
  urutan tetap benar, memetakan id sementara ke id server (`translate`), lalu
  menyegarkan daftar dari server. Sisa antrean dibaca ulang dari IndexedDB, bukan
  dari salinan di memori.
* **Protokol `/api/ws`.** Klien→server: `{"type":"subscribe"}` (minta snapshot),
  `{"type":"ping"}`; teks bebas tetap digemakan. Server→klien:
  `{"type":"hello"|"snapshot"|"todo"|"pong"|"echo"}`, dan setiap mutasi REST
  disiarkan ke semua klien lewat hub `tokio::broadcast`. Tab lain menerima
  `{"type":"todo","event":…}` dan memperbarui daftarnya tanpa refetch.
* **Halaman `/offline`** kini menampilkan salinan lokal (snapshot + jumlah mutasi
  yang menunggu) supaya rute yang belum pernah dibuka pun tidak menjadi jalan buntu.

### Hasil pengukuran

Diukur dengan Chromium headless (`playwright-core`, viewport 390 × 780, emulasi
sentuh) terhadap build **release** di mesin pengembangan yang sama; angka adalah
milidetik. Harness-nya menyalakan biner rilis sendiri (port 3100, database
terpisah) lalu menjalankan skenario: buka `/todos`, matikan server, buat 10 todo
offline, hidupkan server lagi.

| Pengukuran | p50 | p95 |
| --- | --- | --- |
| Snapshot IndexedDB — tulis 200 todo (21 KB) | 0,3 | 1,9 |
| Snapshot IndexedDB — baca 200 todo | 0,2 | 0,3 |
| `GET /todos` (SSR + SQLite, lewat service worker) | 2,2 | 2,6 |
| RTT WebSocket `ping` → `pong` | 0,2 | 0,3 |

| Hidrasi & cat | ms |
| --- | --- |
| TTFB dokumen `/todos` | 3 |
| `DOMContentLoaded` | 80 |
| `first-contentful-paint` | 100 |
| Unduh + instansiasi Wasm | 74 |

| Pemulihan setelah offline (10 mutasi tertunda) | ms |
| --- | --- |
| Server hidup → socket tersambung lagi (termasuk backoff) | 1450 |
| Socket tersambung → antrean kosong (replay 10 mutasi) | 83 |
| Total | 1528 |

Bacaan: menulis satu baris ke IndexedDB ~25× lebih murah daripada satu mutasi
lewat server function + SQLite (±8 ms per mutasi termasuk HTTP, transaksi, dan
siaran WebSocket), jadi antrean lokal memang jalur cepat — dan itulah alasan
mutasi selalu masuk antrean dulu, online maupun offline.

## Catatan perilaku

* **Service worker hanya aktif di build release.** Di build debug, `cargo leptos
  watch` menyajikan aset yang berubah setiap rebuild, sementara aturan navigasi
  service worker meng-cache halaman. Karena itu build debug justru membongkar
  worker dan cache yang tertinggal dari build release sebelumnya. Logikanya ada di
  [`app/src/service_worker.rs`](./app/src/service_worker.rs).
  Untuk menguji perilaku offline, jalankan build release.
* **Nama berkas halaman boleh mengandung `[...]`.** `[id].rs` dideklarasikan lewat
  `#[path = ".../[id].rs"] pub mod …` di `$OUT_DIR/routes.rs`, jadi tidak perlu
  nama modul yang berbeda dari nama rute.
* **Nama berkas bundle tidak di-hash.** `hash-files` sengaja dimatikan: dengan
  hashing, rebuild CSS inkremental saat `cargo leptos watch` berhenti memperbarui
  nama berkas sehingga HTML menunjuk CSS basi. Sebagai gantinya server mengirim
  `Cache-Control: public, max-age=0, must-revalidate` untuk `/pkg/*` — bundle
  600 KB tetap tidak diunduh ulang selama ETag-nya cocok, tapi deploy baru
  langsung terpakai. Cache offline jangka panjang adalah tugas service worker.
* **`/pkg/` dilayani network-first oleh service worker**, bukan
  *stale-while-revalidate*. Karena nama berkas bundle tidak di-hash, SWR bisa
  memasangkan wasm lama dengan HTML baru setelah deploy. Konsekuensinya hanya
  satu revalidasi `304` per muat halaman; saat offline, salinan cache tetap
  dipakai.
* **Baca data lewat GET, tulis lewat POST.** Server function baca
  (`list_todos`, `get_todo`, `get_todo_stats`) memakai
  `#[server(..., input = GetUrl)]` supaya bisa di-cache service worker — Cache API
  memang menolak menyimpan request POST. Fungsi tulis tetap POST sehingga tidak
  pernah ikut ter-cache.
* **Service worker memanaskan shell dan dokumen rute.** Saat install, worker
  mengambil `/pkg/<nama>.js|.wasm|.css` (nama berkas dibaca dari
  `[package.metadata.leptos]`) beserta potongan `snippets/*` yang di-import berkas
  JS, lalu menyimpannya ke cache runtime. Permintaan pertama sebelum worker aktif
  tidak pernah lewat worker, jadi tanpa langkah ini halaman offline hanya bisa
  memuat HTML tanpa Wasm. Worker juga menyimpan dokumen rute yang sedang dibuka
  (`sayagi:warm-document`): router klien tidak pernah meminta dokumen saat
  berpindah halaman, sehingga menyimpan dokumen rute inilah yang membuat reload
  offline bekerja. Semua kegagalan pemanasan diabaikan agar install tidak batal.
* **Yang tersedia saat offline:** dokumen rute yang pernah dibuka (utuh dengan data
  hasil SSR), seluruh bundle `/pkg/*` beserta `snippets/`, ikon, dan response GET
  yang pernah diambil. Halaman yang belum pernah dibuka jatuh ke `/offline`, yang
  kini menampilkan salinan lokal dari IndexedDB; data yang belum pernah diambil
  tampil kembali saat online.
* **`Resource` harus dibaca di dalam batas `<Suspense>`/`<Transition>`.**
  Membaca `resource.get()` di dalam `Signal::derive` saat hidrasi membuat render
  awal klien berbeda dari HTML SSR (fallback vs daftar), dan tachys membatalkan
  hidrasi dengan `Unrecoverable hydration error`. Store klien karena itu baru
  mengambil alih daftar *setelah* batas Suspense selesai (`store.ready()`).
* **Jangan memakai sinyal lokal (`RwSignal::new_local`) di komponen yang juga
  dirender saat SSR.** Nilainya dibungkus `SendWrapper`, dan pembersihan owner
  bisa berjalan di thread runtime Axum yang lain sehingga proses panik
  (`Dropped SendWrapper<T> variable from a thread different to the one it has been
  created with`) — respons terpotong di tengah jalan dan klien menerima HTML
  setengah jadi. `BottomSheet` menyimpan elemen fokus di `Rc<RefCell<…>>` biasa.
* **Swipe-back shell vs gestur bawaan browser.** Lihat bagian *Mobile shell*:
  `overscroll-behavior-x: none` wajib ada agar Chrome Android tidak melakukan
  navigasi mundur sendiri di samping `history.back()` milik shell.
* **Versi cache ikut berubah setiap kali sumber aplikasi berubah.** `sw.js`
  memakai `sayagi-<hash>` sebagai nama cache, dan hash-nya dihitung dari isi
  `Sayagi.toml` + pohon sumber aplikasi (bukan hanya konfigurasi), sehingga deploy
  berisi perubahan kode tidak akan menyajikan HTML lama berdampingan dengan Wasm
  baru. Entri precache juga memakai revisi yang sama.
* **`<A>` tidak menerima atribut tambahan** di leptos_router 0.8. Karena itu
  komponen tombol (`LinkButton`) menaruh gaya di elemen pembungkus dan CSS
  menyasar `.sy-btn > a`; status aktif navigasi bawah memakai
  `aria-current="page"` yang dipasang router sendiri.

## Deviasi dari rencana awal

* **Service worker** digenerate sebagai berkas JS klasik
  ([template](./crates/sayagi-build/src/templates/sw.js)), bukan dikompilasi ke Wasm —
  spesifikasi Web Platform mewajibkan service worker berupa worker JavaScript.
  Sisi Rust tetap yang memasang dan mengelola versinya
  ([`app/src/service_worker.rs`](./app/src/service_worker.rs)).
* **Workbox di-vendor, tidak dari CDN.**
  [`crates/sayagi-build/vendor/workbox/`](./crates/sayagi-build/vendor/workbox/NOTICE.md)
  menyimpan runtime resmi Workbox 7.4.1 (MIT); `sayagi-build` menyalinnya ke
  `public/workbox/` setiap build dan `sw.js` memuatnya lewat
  `importScripts("/workbox/workbox-sw.js")`. Instalasi worker jadi tetap jalan
  saat perangkat offline dan tidak bergantung pada domain pihak ketiga.
* **`sqlx::query_as!`** diganti `sqlx::query_as` / `sqlx::query` versi runtime agar
  build tidak membutuhkan `DATABASE_URL` saat kompilasi.
* Ikon aplikasi digenerate (PNG murni tanpa dependensi) sehingga proyek baru langsung
  bisa dipasang ke home screen tanpa menyertakan berkas biner di template.

## Verifikasi

```bash
# Unit test generator (rute, PNG, template, output Workbox) dan CLI
cargo test --workspace --features ssr

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
curl -s localhost:3000/todos | grep sy-todo__title             # SSR + SQLite
curl -sI localhost:3000/manifest.webmanifest                    # manifest hasil generate
curl -sI localhost:3000/sw.js                                   # service worker hasil generate
curl -s "localhost:3000/api/list_todos4286897385061139490"      # GET baca (bisa di-cache)
curl -sI localhost:3000/workbox/workbox-sw.js                   # runtime Workbox yang di-vendor
curl -s localhost:3000/workbox/workbox-precaching.prod.js | wc -c
curl -sI localhost:3000/icons/icon-192.png                      # ikon hasil generate
```

Endpoint WebSocket contoh (dari diagram arsitektur) tersedia di `ws://127.0.0.1:3000/api/ws`.
Kirim `{"type":"subscribe"}` untuk snapshot, `{"type":"ping"}` untuk pong, atau teks
bebas yang akan digemakan:

```bash
websocat ws://127.0.0.1:3000/api/ws   # {"type":"subscribe"} → {"type":"snapshot","todos":[…]}
```

Perilaku mobile shell dan lapisan offline/sinkronisasi diverifikasi di Chromium sungguhan (bukan unit test) karena
menyangkut gestur sentuh, View Transitions, IndexedDB, dan service worker. Skenario
yang dijalankan pada build release: buka/tutup bottom sheet (drag, backdrop,
`Escape`, kunci scroll, `aria-pressed` filter), tambah todo lewat sheet lalu reload,
hitung pemanggilan `document.startViewTransition`, swipe-back dari tepi kiri,
sinkronisasi dua tab lewat WebSocket, lalu matikan server: mutasi masuk antrean
IndexedDB dan tampil optimistis, reload offline memuat dokumen + Wasm dari cache
dan daftar dari snapshot, dan hidupkan server lagi untuk memastikan antrean diputar
ulang sampai database server. Semuanya 24/24 lulus. Arah animasi transisi diuji
terpisah dengan membaca nama animasi lapisan view transition saat transisi
berjalan: masuk anak halaman (`sy-nav-child`, `sy-page-in-right`), naik ke induk
(`sy-nav-parent`, `sy-page-in-left`, termasuk lewat swipe-back), dan selevel
(`sy-nav-level`, fade) — 11/11 lulus, termasuk cek tidak ada view transition yang
dibatalkan. Jalur cadangan diuji dengan menghapus `document.startViewTransition`
dari halaman: arahnya tetap benar (`sy-no-vt` + kelas arah) — 4/4 lulus. Angka
gerak Material diuji dengan membekukan transisi pada waktu tertentu lalu membaca
durasi, easing, dan opasitas lapisannya (fade masuk terbaca 0,444 → 0,889, fade
halaman lama 0; geser 450 ms, fade 337,5 ms) — 13/13 lulus. Stabilitas chrome
diuji dengan mengukur warna rata-rata area nav, header, dan banner offline dari
screenshot pada enam titik waktu transisi: selisih 0 (dari 255) di semua titik,
untuk masuk anak halaman, naik ke induk, dan pindah selevel — 13/13 lulus.
Sebelum perbaikan, area nav berubah dari `116,33,158` menjadi `208,11,53` di
tengah transisi (selisih 105 pada kanal biru) — itulah kedipan yang dilaporkan.
