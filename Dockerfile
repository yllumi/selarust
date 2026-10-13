# syntax=docker/dockerfile:1.7
#
# Image produksi Selarust: binary Axum + bundle Wasm + aset PWA.
#
#   docker build -t selarust-todo .
#   docker run --rm -p 8080:3000 -v selarust-data:/data selarust-todo
#
# Atau lewat compose: `docker compose up --build` (lihat docker-compose.yml,
# yang menerima environment variable — daftarnya di .env.example).

# ---------------------------------------------------------------------------
# Tahap 1 — builder: seluruh toolchain + pipeline `cargo leptos`.
# ---------------------------------------------------------------------------
FROM rust:1.99-slim-bookworm AS builder

# `rust-toolchain.toml` meminta channel `stable`; image ini sudah menyediakan
# 1.99.0 sebagai toolchain bawaan. Menyetel RUSTUP_TOOLCHAIN membuat rustup
# memakai toolchain itu apa adanya, sehingga tidak mengunduh salinan `stable`
# kedua. Kalau versi toolchain dinaikkan, samakan tag image di atas dengan
# nilai ini.
ENV RUSTUP_TOOLCHAIN=1.99.0

# Target Wasm untuk sisi klien.
RUN rustup target add wasm32-unknown-unknown

# `cc` dibutuhkan karena SQLite dikompilasi dari sumber (libsqlite3-sys dengan
# feature `bundled`), sehingga binary tidak bergantung pada libsqlite3 sistem.
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential curl pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Versi disamakan dengan `cargo leptos --version` di mesin pengembangan supaya
# perilaku build (termasuk unduhan wasm-bindgen-cli) tidak berbeda.
RUN cargo install cargo-leptos --version 0.3.11 --locked

WORKDIR /app
COPY . .

# Registry dan target sebagai cache BuildKit: rebuild berikutnya hanya
# mengompilasi crate yang berubah. Hasil akhirnya disalin ke /out di dalam RUN
# yang sama, karena isi cache mount tidak ikut tersimpan ke layer image.
# Aset PWA (manifest, service worker, ikon, Workbox) ditulis `build.rs`, dan
# `cargo leptos` menyalin direktori aset ke site root **sebelum** build berjalan.
# Dalam build ber-cache, cargo bisa melewati `build.rs` sepenuhnya (fingerprint
# masih dianggap segar) sehingga asetnya tidak pernah ditulis dan site root
# kehilangan manifest serta service worker. Karena itu generasinya dijalankan
# lebih dulu dan eksplisit — perintah yang sama dengan `selarust pwa`.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo run --release --package selarust-cli --bin selarust -- pwa --dir app \
    && cargo leptos build --release \
    && mkdir -p /out \
    && cp target/release/mobile-pwa /out/mobile-pwa \
    && cp -r target/site /out/site

# ---------------------------------------------------------------------------
# Tahap 2 — runtime: hanya binary + aset situs.
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# curl dipakai healthcheck; ca-certificates untuk permintaan HTTPS keluar.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin selarust \
    && mkdir -p /data \
    && chown selarust:selarust /data

WORKDIR /app

COPY --from=builder --chown=selarust:selarust /out/mobile-pwa /app/mobile-pwa
COPY --from=builder --chown=selarust:selarust /out/site /app/site

# Kontrak runtime Leptos. Nilai ini sengaja eksplisit: `get_configuration`
# membacanya dari environment, bukan dari metadata Cargo.toml (yang hanya
# tersedia saat `cargo leptos serve/watch`).
ENV LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_OUTPUT_NAME=mobile-pwa \
    LEPTOS_ENV=PROD \
    SELARUST_DATABASE_URL=sqlite:///data/selarust.db

# SQLite ditulis ke volume supaya data bertahan saat container diganti.
VOLUME ["/data"]

USER selarust
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:3000/ >/dev/null || exit 1

CMD ["/app/mobile-pwa"]
