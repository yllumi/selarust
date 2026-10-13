# Instruksi kerja untuk agen AI (Copilot) — Sayagi Rust

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
cargo run --bin sayagi -- routes --dir app    # rute hasil pemindaian app/src/pages
cargo run --bin sayagi -- pwa --dir app       # regenerate manifest, sw.js, ikon
```

`--dir` menunjuk direktori yang memuat `Sayagi.toml`; dari root workspace selalu
pakai `--dir app`, sedangkan di proyek hasil `sayagi new` bawaan CWD sudah benar.

* Build/verifikasi jangan mengganggu dev server pengguna di `:3000`. Untuk
  pengujian, jalankan build release dengan port, situs, dan database terpisah:
  `LEPTOS_SITE_ADDR`, `LEPTOS_SITE_ROOT`, `SAYAGI_DATABASE_URL`.
* `cargo leptos build` dengan `LEPTOS_SITE_ROOT` alternatif menaruh `app/public/*`
  di site-root metadata (`target/site`), jadi salin ulang `app/public` ke situs uji
  sebelum menjalankan probe.

## Aturan yang tidak boleh dilanggar

1. **Jangan menyunting berkas hasil generate**: `app/public/manifest.webmanifest`,
   `app/public/sw.js`, `app/public/icons/`, `app/public/workbox/`, serta
   `$OUT_DIR/{routes.rs,config.rs}`. Ubah `app/Sayagi.toml` atau generator di
   `crates/sayagi-build/src/**`, lalu build ulang.
2. **Jaga `app/` dan `crates/sayagi-cli/templates/app/` tetap sinkron.** Keduanya
   memuat berkas yang sama (termasuk `style/main.css`); perubahan pada satu sisi
   harus dicerminkan di sisi lain.
3. **Hormati batas crate.** Primitif generik → `crates/sayagi`; domain produk →
   `app`. `crates/sayagi-build` dipanggil dari `build.rs`, jadi tidak boleh
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
9. **CSS: kelas berprefix `sy-`.** Warna, token, dan seluruh aturan transisi hanya
   di `app/style/main.css`. Elemen chrome shell (`sy-chrome-header`,
   `sy-chrome-banner`, `sy-chrome-nav`) wajib punya `view-transition-name` sendiri
   dan tetap `animation: none`; gerak halaman memakai token `--sy-motion-*`
   (angka Material 3). Nilai animasi jangan di-hardcode di komponen.
10. **Workbox vendored apa adanya.** Jangan menyunting
    `crates/sayagi-build/vendor/workbox/**`; ikuti prosedur di `NOTICE.md`.
11. **`hash-files` tetap mati** pada `[package.metadata.leptos]`; alasannya
    tertulis di `app/Cargo.toml` (rebuild CSS inkremental akan menunjuk berkas basi).
12. **Tanpa rahasia di repo.** Kredensial/URL lewat environment
    (`SAYAGI_DATABASE_URL`). Jangan commit `target/`, `*.db*`, atau aset generate.

## Gaya kode

* Rust standar (`cargo fmt`), tanpa menambah peringatan `clippy`.
* Bahasa komentar mengikuti lokasi: `crates/sayagi*` memakai doc comment
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
* Perubahan dokumen saja tidak butuh build.

## Dokumentasi

| Perubahan | Perbarui |
| --- | --- |
| Arsitektur, kontrak antar-lapisan, invarian | [ARCHITECTURE.md](../ARCHITECTURE.md) |
| Perilaku, cara pakai, angka hasil ukur, hasil verifikasi | [README.md](../README.md) |
| Aturan kerja agen | berkas ini |
