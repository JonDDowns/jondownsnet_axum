pub mod config;
pub mod posts;
use crate::config::*;
use crate::posts::*;

use axum::{
    extract::Host,
    handler::HandlerWithoutStateExt,
    http::{StatusCode, Uri},
    response::{IntoResponse, Redirect},
    routing::get,
    BoxError, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use std::{net::SocketAddr, path::PathBuf};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ports {
    http: u16,
    https: u16,
}

#[tokio::main]
async fn main() {
    let mystr: String = env::args().nth(1).expect("No config file path given");
    let cfg: Config = read_cfg(&mystr);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "jdnet=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let ports = Ports {
        http: cfg.port_http,
        https: cfg.port_https,
    };
    tokio::spawn(redirect_http_to_https(ports));

    let tlscfg = RustlsConfig::from_pem_file(
        PathBuf::from(&cfg.pem_dir).join("cert.pem"),
        PathBuf::from(&cfg.pem_dir).join("privkey.pem"),
    )
    .await
    .unwrap();

    let db_connection_str = std::env::var("DATABASE_URL").unwrap_or_else(|_| cfg.cnxn_str);

    // set up connection pool
    let pool = PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_connection_str)
        .await
        .expect("can't connect to database");

    let posts = sqlx::query_as::<_, Post>(
        "SELECT post_id
        , post_date
        , post_title
        , post_body
        , post_summary 
        , post_thumbnail
        , post_thumbnail_alt
        from posts
        ORDER BY post_date",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let shared_state = Arc::new(posts);

    // build our application with some routes
    let app = Router::new()
        .route("/", get(index))
        .route("/posts/:post_id", get(post))
        .nest_service("/assets", ServeDir::new("assets"))
        .nest_service("/.well-known/", ServeDir::new(".well-known"))
        .with_state(shared_state);
    let app = app.fallback(handler_404);

    // run it with hyper
    let listener = SocketAddr::from(([0, 0, 0, 0], ports.https));
    tracing::debug!("listening on {}", listener);
    axum_server::bind_rustls(listener, tlscfg)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "404: The resource you have requested could not be found.",
    )
}

#[allow(dead_code)]
async fn redirect_http_to_https(ports: Ports) {
    fn make_https(host: String, uri: Uri, ports: Ports) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host = host.replace(&ports.http.to_string(), &ports.https.to_string());
        parts.authority = Some(https_host.parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, ports) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], ports.http));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, redirect.into_make_service())
        .await
        .unwrap();
}
