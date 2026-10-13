//! Menjalankan generator Selarust pada setiap build: rute file-based, konstanta
//! konfigurasi, manifest PWA, service worker, dan ikon.

fn main() {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR diset oleh Cargo");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR diset oleh Cargo");

    match selarust_build::generate(manifest_dir, out_dir) {
        Ok(report) => println!("cargo:warning=selarust: {}", report.summary()),
        Err(error) => panic!("generasi Selarust gagal: {error}"),
    }
}
