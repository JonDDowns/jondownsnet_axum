pub mod config;
pub mod posts;
pub mod routing;
use crate::config::*;
use crate::posts::*;
use crate::routing::*;
use axum::http::header;
use axum::http::HeaderValue;
use axum::{routing::get, Router};
use axum_server::tls_rustls::RustlsConfig;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use std::{net::SocketAddr, path::PathBuf};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::{compression::CompressionLayer, set_header::SetResponseHeaderLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    let compression_layer: CompressionLayer = CompressionLayer::new()
        .br(true)
        .deflate(true)
        .gzip(true)
        .zstd(true);

    let cc = ServiceBuilder::new().layer(SetResponseHeaderLayer::appending(
        header::CACHE_CONTROL,
        HeaderValue::from_static("max-age=604800"),
    ));

    // build our application with some routes
    let app = Router::new()
        .route("/", get(index))
        .route("/posts/:post_id", get(post))
        .nest_service("/assets", ServeDir::new("assets"))
        .nest_service("/.well-known/", ServeDir::new(".well-known"))
        .with_state(shared_state)
        .layer(cc)
        .layer(compression_layer);
    let app = app.fallback(handler_404);

    // run it with hyper
    let listener = SocketAddr::from(([0, 0, 0, 0], ports.https));
    tracing::debug!("listening on {}", listener);
    axum_server::bind_rustls(listener, tlscfg)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
