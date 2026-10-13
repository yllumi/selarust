//! Binary Axum: SSR, hidrasi, server functions, aset PWA, dan WebSocket sync.

#[cfg(feature = "ssr")]
use axum::extract::ws::WebSocketUpgrade;

/// State yang dibagikan ke handler Axum.
///
/// `LeptosOptions` ikut di dalamnya karena `leptos_axum` mengambilnya lewat
/// `FromRef`; pool SQLite dan hub sinkronisasi disediakan ke server functions
/// sebagai context Leptos.
#[cfg(feature = "ssr")]
#[derive(Clone, axum::extract::FromRef)]
struct AppState {
    leptos_options: leptos::prelude::LeptosOptions,
    pool: mobile_pwa::db::Pool,
    hub: mobile_pwa::sync::SyncHub,
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::routing::get;
    use axum::{middleware, Router};
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use tower_http::compression::CompressionLayer;

    use mobile_pwa::app::{shell, App};
    use mobile_pwa::{config, db, sync};

    let conf = get_configuration(None).expect("konfigurasi Leptos");
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;

    let database_url = db::resolve_url(
        &std::env::var("SAYAGI_DATABASE_URL")
            .unwrap_or_else(|_| config::DATABASE_URL.to_string()),
    );
    let pool = db::connect(
        &database_url,
        config::DATABASE_MAX_CONNECTIONS,
        config::DATABASE_SEED,
    )
    .await
    .expect("membuka SQLite tertanam");
    log!("database siap: {database_url}");

    // Rute Leptos diturunkan dari <Routes> yang digenerate `sayagi-build`.
    let routes = generate_route_list(App);

    let state = AppState {
        leptos_options,
        pool,
        hub: sync::SyncHub::new(),
    };

    let app = Router::new()
        .route("/api/ws", get(ws_handler))
        .leptos_routes_with_context(
            &state,
            routes,
            {
                let pool = state.pool.clone();
                let hub = state.hub.clone();
                move || {
                    provide_context(pool.clone());
                    provide_context(hub.clone());
                }
            },
            {
                let leptos_options = state.leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        // Tipe state ditulis eksplisit: `FromRef` dihasilkan untuk setiap field,
        // jadi tanpa anotasi ini `S` ambigu.
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        // Kompresi lebih dulu, lalu middleware cache: dengan urutan ini middleware
        // melihat respons final (sudah ter-encode) sehingga ETag-nya berbeda per
        // representasi gzip/identity.
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(asset_cache_control))
        .with_state(state);

    log!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind alamat");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("menjalankan server");
}

/// Header cache + validasi untuk bundle di `/pkg`.
///
/// Nama berkas di sana tidak di-hash, jadi `immutable` justru berbahaya (deploy
/// berikutnya tertahan di cache browser). Yang dipakai adalah
/// `max-age=0, must-revalidate`: setiap muat ulang halaman menanyakan versi
/// terbaru, tetapi jawabannya cukup `304 Not Modified` sehingga bundle Wasm
/// ~600 KB tidak diunduh ulang.
///
/// ETag-nya dihitung di sini karena `leptos_axum::file_and_error_handler`
/// membangun ulang request saat menyajikan berkas statis dan hanya meneruskan
/// `Accept-Encoding` — header `If-None-Match` dari browser tidak pernah sampai
/// ke lapisan penyajian berkas.
#[cfg(feature = "ssr")]
async fn asset_cache_control(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{header, HeaderValue, StatusCode};

    let is_bundle = request.uri().path().starts_with("/pkg/");
    let if_none_match = request.headers().get(header::IF_NONE_MATCH).cloned();
    let mut response = next.run(request).await;

    if !is_bundle {
        return response;
    }

    const POLICY: &str = "public, max-age=0, must-revalidate";
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(POLICY));

    let Some(validator) = bundle_validator(&response) else {
        return response;
    };
    let Ok(etag) = HeaderValue::from_str(&validator) else {
        return response;
    };

    if if_none_match.as_ref() == Some(&etag) {
        let mut not_modified = axum::response::Response::new(axum::body::Body::empty());
        *not_modified.status_mut() = StatusCode::NOT_MODIFIED;
        let headers = not_modified.headers_mut();
        headers.insert(header::ETAG, etag);
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(POLICY));
        if let Some(last_modified) = response.headers().get(header::LAST_MODIFIED) {
            headers.insert(header::LAST_MODIFIED, last_modified.clone());
        }
        if let Some(vary) = response.headers().get(header::VARY) {
            headers.insert(header::VARY, vary.clone());
        }
        return not_modified;
    }

    response.headers_mut().insert(header::ETAG, etag);
    response
}

/// Turunan ETag dari metadata berkas yang sudah dikirim penyaji statis.
///
/// `Last-Modified` berubah begitu isi berkas berubah, dan `content-encoding`
/// ikut dihitung supaya representasi gzip dan identity tidak berbagi ETag
/// (responsnya `Vary: accept-encoding`).
#[cfg(feature = "ssr")]
fn bundle_validator(response: &axum::response::Response) -> Option<String> {
    use axum::http::header;

    let last_modified = response.headers().get(header::LAST_MODIFIED)?.to_str().ok()?;
    let encoding = response
        .headers()
        .get(header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("identity");
    let length = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("-");

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in [last_modified, encoding, length] {
        for byte in part.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(format!("\"{hash:016x}\""))
}

/// WebSocket `/api/ws`: snapshot awal, siaran mutasi, dan echo teks bebas.
#[cfg(feature = "ssr")]
async fn ws_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    upgrade: WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    upgrade.on_upgrade(move |socket| {
        mobile_pwa::sync::handle_socket(socket, state.hub, state.pool)
    })
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // Tidak ada entrypoint klien: hidrasi dimulai oleh `hydrate()` di lib.rs.
}
