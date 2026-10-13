# Instruksi kerja untuk agen AI (Copilot) — Selarust

Metaframework PWA mobile-first di Rust: satu basis kode dikompilasi dua kali —
binary **Axum** (SSR + server functions) dan modul **Wasm/Leptos** (hidrasi).
Contoh aplikasi di `app/` adalah CRUD todo yang juga menjadi cetakan proyek baru.

Sebelum mengubah apa pun, baca [ARCHITECTURE.md](../ARCHITECTURE.md) (lapisan,
kontrak, invarian) dan [README.md](../README.md) (cara menjalankan, catatan
perilaku, hasil pengukuran).

## Perintah

```bash
. "$HOME/.cargo/env"                          # cargo tidak ada di PATH sesi default
cargo test --workspace --features ssr         # unit test (generator, store, CLI, config)
cargo check -p mobile-pwa --features ssr
cargo check -p mobile-pwa --target wasm32-unknown-unknown --no-default-features --features hydrate
cargo leptos build --release                  # binary Axum + bundle Wasm + aset PWA
cargo leptos watch                            # dev server :3000
cargo run --bin selarust -- routes --dir app    # rute hasil pemindaian app/src/pages
cargo run --bin selarust -- pwa --dir app       # regenerate manifest, sw.js, ikon
```

`--dir` menunjuk direktori yang memuat `Selarust.toml`; dari root workspace selalu
pakai `--dir app`, sedangkan di proyek hasil `selarust new` bawaan CWD sudah benar.

```bash
docker compose up --build        # container produksi di http://127.0.0.1:8080
docker compose logs -f app
docker compose down              # data tetap tersimpan di volume `selarust-data`
docker build -t selarust-todo .   # image saja, tanpa compose
```

* Build/verifikasi jangan mengganggu dev server pengguna di `:3000`. Untuk
  pengujian, jalankan build release dengan port, situs, dan database terpisah:
  `LEPTOS_SITE_ADDR`, `LEPTOS_SITE_ROOT`, `SELARUST_DATABASE_URL`.
* `cargo leptos build` dengan `LEPTOS_SITE_ROOT` alternatif menaruh `app/public/*`
  di site-root metadata (`target/site`), jadi salin ulang `app/public` ke situs uji
  sebelum menjalankan probe.

## Aturan yang tidak boleh dilanggar

1. **Jangan menyunting berkas hasil generate**: `app/public/manifest.webmanifest`,
   `app/public/sw.js`, `app/public/icons/`, `app/public/workbox/`, serta
   `$OUT_DIR/{routes.rs,config.rs}`. Ubah `app/Selarust.toml` atau generator di
   `crates/selarust-build/src/**`, lalu build ulang.
2. **Jaga `app/` dan `crates/selarust-cli/templates/app/` tetap sinkron.** Keduanya
   memuat berkas yang sama (termasuk `style/main.css`); perubahan pada satu sisi
   harus dicerminkan di sisi lain.
3. **Hormati batas crate.** Primitif generik → `crates/selarust`; domain produk →
   `app`. `crates/selarust-build` dipanggil dari `build.rs`, jadi tidak boleh
   menambah dependensi Leptos/Tokio/pesawat berat lain di sana.
4. **Disiplin fitur.** Kode server di balik `#[cfg(feature = "ssr")]`, kode
   browser di balik `#[cfg(feature = "hydrate")]`. Setiap perubahan harus lolos
   `cargo check` untuk **kedua** sisi (ssr dan wasm32/hydrate).
5. **Halaman = berkas.** Tambah rute dengan menambah berkas di `app/src/pages/**`
   yang mengekspor `#[component] pub fn Page() -> impl IntoView`; jangan menulis
   rute manual di kode.
6. **Server function, bukan REST manual.** Pakai `#[server(Nama, "/api")]` di
   `app/src/api.rs`. Fungsi baca memakai `input = GetUrl` supaya bisa di-cache
   service worker; fungsi tulis tetap POST. Validasi input hanya di sisi server.
7. **Mutasi dari UI lewat antrean.** `TodoStore::enqueue(TodoMutation)` — jangan
   memanggil server function tulis langsung dari halaman/komponen.
8. **Satu penulis aturan event.** Menambah jenis perubahan berarti menambah
   varian `TodoEvent` **dan** cabangnya di `TodoEvent::apply` (satu tempat),
   lalu disiarkan lewat `SyncHub::publish` di server function.
9. **CSS: kelas berprefix `sr-`.** Warna, token, dan seluruh aturan transisi hanya
   di `app/style/main.css`. Elemen chrome shell (`sr-chrome-header`,
   `sr-chrome-banner`, `sr-chrome-nav`) wajib punya `view-transition-name` sendiri
   dan tetap `animation: none`; gerak halaman memakai token `--sr-motion-*`
   (angka Material 3). Nilai animasi jangan di-hardcode di komponen.
10. **Workbox vendored apa adanya.** Jangan menyunting
    `crates/selarust-build/vendor/workbox/**`; ikuti prosedur di `NOTICE.md`.
11. **Aset PWA dipublikasikan generator, bukan cargo-leptos.** `cargo leptos`
    menyalin `assets-dir` ke site root *sebelum* `cargo build`, jadi manifest,
    `sw.js`, ikon, dan Workbox yang dibuat `build.rs` pada build yang sama tidak
    ikut tersalin. Itu ditangani
    [`crates/selarust-build/src/site.rs`](../crates/selarust-build/src/site.rs) lewat
    `LEPTOS_SITE_ROOT`. Jangan mengganti mekanisme ini dengan salinan manual di
    `build.rs` — dan jangan menghapus publikasinya, karena checkout baru
    (termasuk image Docker) akan kehilangan manifest dan service worker. Karena
    build ber-cache bisa melewati `build.rs` sama sekali, build container
    menjalankan `selarust pwa --dir app` lebih dulu (langkah di `Dockerfile`);
    jangan menghapus langkah itu.
12. **`hash-files` tetap mati** pada `[package.metadata.leptos]`; alasannya
    tertulis di `app/Cargo.toml` (rebuild CSS inkremental akan menunjuk berkas basi).
13. **Tanpa rahasia di repo.** Kredensial/URL lewat environment
    (`SELARUST_DATABASE_URL`). Jangan commit `target/`, `*.db*`, atau aset generate.
14. **Kontrak container dijaga bersama.** Runtime membaca `LEPTOS_*` dan
    `SELARUST_DATABASE_URL` dari environment (bukan metadata Cargo). Menambah
    variabel, aset, atau port baru berarti mengubah [`Dockerfile`](../Dockerfile),
    [`docker-compose.yml`](../docker-compose.yml),
    [`.env.example`](../.env.example), dan bagian Docker di README sekaligus.
    Semua knob compose memakai pola `${NAMA:-bawaan}` supaya bisa ditimpa dari
    shell/`.env`; jangan menulis nilai mati di sana — kecuali **port container
    (`3000`)**, yang memang statis karena terikat `EXPOSE` di Dockerfile dan
    `LEPTOS_SITE_ADDR`; hanya port host yang boleh diatur lewat variable. Container berjalan tanpa root
    dengan filesystem read-only: hanya `/data` (SQLite, volume) dan `/tmp` yang
    boleh ditulis. Jangan commit `.env` (sudah di `.gitignore`); contoh yang boleh
    dibagikan ada di `.env.example`.
15. **`.dockerignore` sengaja membuang `target/`, `*.db*`, `*.md`, dan aset PWA
    hasil generate** (dibuat ulang `build.rs` di dalam container). Jangan
    menambahkan pengecualian yang menendang berkas yang dibutuhkan build seperti
    `app/style/`, `app/public/robots.txt`, atau `crates/selarust-build/vendor/`.

## Gaya kode

* Rust standar (`cargo fmt`), tanpa menambah peringatan `clippy`.
* Bahasa komentar mengikuti lokasi: `crates/selarust*` memakai doc comment
  **Inggris**, `app/**` memakai **Indonesia**.
* Komentar menjelaskan *mengapa* (keputusan, jebakan, alasan angka), bukan
  *apa* yang sudah terbaca dari kode. Jangan mengomentari baris yang jelas.
* Dokumen dan balasan ke pengguna berbahasa Indonesia.

## Verifikasi

* Perubahan Rust: minimal `cargo test --workspace --features ssr`, plus
  `cargo check` dua sisi bila menyentuh kode bersama.
* Perubahan UI, gestur, service worker, offline, atau WebSocket: verifikasi di
  browser sungguhan (headless Chromium/Playwright) terhadap **build release** di
  port terpisah dari `:3000`, bukan sekadar membaca kode. Lakukan hard reload:
  `cargo leptos watch` mengganti CSS tetapi tidak selalu Wasm.
* Perubahan pada blok transisi/chrome: jalankan ulang probe transisi, karena
  kesalahan di sini mudah muncul sebagai kedipan chrome yang hanya terlihat pada
  animasi sungguhan.
* Perubahan `Dockerfile`, `docker-compose.yml`, `.dockerignore`, atau daftar
  environment variable: buktikan dengan
  build + run sungguhan (`docker compose up --build`), lalu periksa
  `curl -fsS http://127.0.0.1:8080/`, halaman SSR yang membaca SQLite
  (`/todos`), aset PWA (`/sw.js`, `/manifest.webmanifest`, `/pkg/mobile-pwa.wasm`),
  dan data yang bertahan setelah `docker compose down` + `up`. Jangan hanya
  membaca Dockerfile.
* Perubahan dokumen saja tidak butuh build.

## Dokumentasi

| Perubahan | Perbarui |
| --- | --- |
| Arsitektur, kontrak antar-lapisan, invarian | [ARCHITECTURE.md](../ARCHITECTURE.md) |
| Perilaku, cara pakai, angka hasil ukur, hasil verifikasi, cara menjalankan container | [README.md](../README.md) |
| Aturan kerja agen | berkas ini |
