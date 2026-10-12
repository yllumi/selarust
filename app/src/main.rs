//! Binary Axum: SSR, hidrasi, server functions, aset PWA, dan WebSocket.

#[cfg(feature = "ssr")]
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};

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
    use mobile_pwa::{config, db};

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

    let app = Router::new()
        .route("/api/ws", get(ws_handler))
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            {
                let pool = pool.clone();
                move || provide_context(pool.clone())
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(middleware::from_fn(asset_cache_control))
        .layer(CompressionLayer::new())
        .with_state(leptos_options);

    log!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind alamat");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("menjalankan server");
}

/// Memberi header cache panjang untuk bundle Wasm yang sudah di-hash oleh
/// `cargo-leptos`. Tidak diterapkan pada build debug agar hot-reload tetap bekerja.
#[cfg(feature = "ssr")]
async fn asset_cache_control(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let is_bundle = request.uri().path().starts_with("/pkg/");
    let mut response = next.run(request).await;
    if is_bundle && !cfg!(debug_assertions) {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

/// Contekan WebSocket dari diagram arsitektur: `/api/ws` menggemakan pesan.
#[cfg(feature = "ssr")]
async fn ws_handler(upgrade: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    upgrade.on_upgrade(handle_socket)
}

#[cfg(feature = "ssr")]
async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(text) => {
                let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
                let reply = format!(r#"{{"echo":"{escaped}"}}"#);
                if socket.send(Message::Text(reply.into())).await.is_err() {
                    break;
                }
            }
            Message::Binary(bytes) => {
                if socket.send(Message::Binary(bytes)).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // Tidak ada entrypoint klien: hidrasi dimulai oleh `hydrate()` di lib.rs.
}
