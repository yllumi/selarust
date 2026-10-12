//! HTML shell dan komponen akar aplikasi.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, HashedStylesheet, MetaTags, Title};
use leptos_router::components::Router;

use crate::config;
use crate::pages::FileRoutes;

/// Kerangka HTML yang dikirim server sebelum Wasm mengambil alih.
///
/// Semua tag PWA (manifest, theme color, apple-touch-icon) ikut di sini supaya
/// aplikasi bisa dipasang ke home screen. Stylesheet memakai
/// [`HashedStylesheet`] karena `cargo-leptos` meng-hash nama berkas di `/pkg`
/// (lihat `hash-files = true` di `Cargo.toml`).
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang=config::LANG>
            <head>
                <meta charset="utf-8"/>
                <meta
                    name="viewport"
                    content="width=device-width, initial-scale=1, viewport-fit=cover"
                />
                <meta name="theme-color" content=config::THEME_COLOR/>
                <meta name="description" content=config::APP_DESCRIPTION/>
                <meta name="mobile-web-app-capable" content="yes"/>
                <meta name="apple-mobile-web-app-capable" content="yes"/>
                <meta name="apple-mobile-web-app-title" content=config::APP_SHORT_NAME/>
                <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"/>
                <link rel="manifest" href=config::MANIFEST_PATH/>
                <link rel="icon" type="image/png" href=config::PRIMARY_ICON/>
                <link rel="apple-touch-icon" href=config::PRIMARY_ICON/>
                <HashedStylesheet options=options.clone() id="leptos"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// Komponen akar: metadata dan pohon rute hasil generate.
///
/// Stylesheet-nya sudah ditulis `shell`, sehingga navigasi sisi klien tidak
/// perlu memuatnya lagi.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text=config::APP_NAME/>
        <Router>
            <FileRoutes/>
        </Router>
    }
}
