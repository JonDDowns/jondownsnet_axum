pub mod config;
pub mod posts;
use crate::config::*;
use crate::posts::*;
use axum::{
    extract::Host,
    handler::HandlerWithoutStateExt,
    http::{Request, StatusCode, Uri},
    response::{IntoResponse, Redirect},
    routing::get,
    BoxError, Router,
};
use hyper::body::Incoming;

use futures_util::pin_mut;
use hyper_util::rt::{TokioExecutor, TokioIo};
use openssl::ssl::{Ssl, SslAcceptor, SslFiletype, SslMethod};
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tokio_openssl::SslStream;
use tower::Service;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};
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

    let mut tls_builder = SslAcceptor::mozilla_modern_v5(SslMethod::tls()).unwrap();

    tls_builder
        .set_certificate_file(
            PathBuf::from(&cfg.pem_dir).join("cert.pem"),
            SslFiletype::PEM,
        )
        .unwrap();

    tls_builder
        .set_private_key_file(
            PathBuf::from(&cfg.pem_dir).join("privkey.pem"),
            SslFiletype::PEM,
        )
        .unwrap();

    tls_builder.check_private_key().unwrap();

    let tls_acceptor = tls_builder.build();

    let ports = Ports {
        http: cfg.port_http,
        https: cfg.port_https,
    };
    tokio::spawn(redirect_http_to_https(ports));

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
    let bind = "[::1]:3000";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();
    info!("HTTPS server listening on {bind}. To contact curl -k https://localhost:3000");
    pin_mut!(tcp_listener);

    loop {
        let tower_service = app.clone();
        let tls_acceptor = tls_acceptor.clone();

        // Wait for new tcp connection
        let (cnx, addr) = tcp_listener.accept().await.unwrap();

        tokio::spawn(async move {
            let ssl = Ssl::new(tls_acceptor.context()).unwrap();
            let mut tls_stream = SslStream::new(ssl, cnx).unwrap();
            if let Err(err) = SslStream::accept(Pin::new(&mut tls_stream)).await {
                error!(
                    "error during tls handshake connection from {}: {}",
                    addr, err
                );
                return;
            }

            let stream = TokioIo::new(tls_stream);

            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                tower_service.clone().call(request)
            });

            let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(stream, hyper_service)
                .await;

            if let Err(err) = ret {
                warn!("error serving connection from {}: {}", addr, err);
            }
        });
    }
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

    // run it with hyper
    let bind = "[::1]:7878";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();
    tracing::debug!("listening on {}", tcp_listener.local_addr().unwrap());
    axum::serve(tcp_listener, redirect.into_make_service())
        .await
        .unwrap();
}
