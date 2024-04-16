use askama::Template;
use axum::{
    extract::{Host, Path, State},
    handler::HandlerWithoutStateExt,
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    BoxError, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use serde::{Deserialize, Serialize};
use serde_yaml;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::time::Date;
use sqlx::FromRow;
use std::sync::Arc;
use std::{net::SocketAddr, path::PathBuf};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use std::time::Duration;

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ports {
    http: u16,
    https: u16,
}

#[derive(FromRow, Debug, Clone)]
pub struct Post {
    pub post_id: i32,
    pub post_title: String,
    pub post_date: Date,
    pub post_body: String,
    pub post_summary: String,
    pub post_thumbnail: String,
    pub post_thumbnail_alt: String,
}

#[tokio::main]
async fn main() {
    let cfg: Config = read_cfg();
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

    println!("{}", cfg.pem_dir);
    println!("{:?}", tlscfg);

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

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    username: String,
    password: String,
    cnxn_str: String,
    pem_dir: String,
    port_http: u16,
    port_https: u16,
    max_connections: u32,
}

fn read_cfg() -> Config {
    let f = std::fs::File::open("../config_dev.yaml").expect("lul");
    let d: Config = serde_yaml::from_reader(f).expect("Something happened");
    d
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Fialed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}

async fn post(
    Path(post_id): Path<String>,
    State(state): State<Arc<Vec<Post>>>,
) -> impl IntoResponse {
    let mut template = PostTemplate {
        post_title: "none",
        post_date: "none".to_string(),
        post_body: "none",
        post_thumbnail: "none",
        post_thumbnail_alt: "none",
    };
    // if the user's query matches a post title then render a template
    for i in 0..state.len() {
        if post_id == state[i].post_id.to_string() {
            template = PostTemplate {
                post_title: &state[i].post_title,
                post_date: state[i].post_date.to_string(),
                post_body: &state[i].post_body,
                post_thumbnail: &state[i].post_thumbnail,
                post_thumbnail_alt: &state[i].post_thumbnail_alt,
            };
            break;
        }
    }

    if &template.post_title == &"none" {
        return (
            StatusCode::NOT_FOUND,
            "404: The resource you have requested could not be found.",
        )
            .into_response();
    }

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "try again later").into_response(),
    }
}

#[derive(Template)]
#[template(path = "blogtempl.html", escape = "none")]
struct PostTemplate<'a> {
    post_title: &'a str,
    post_date: String,
    post_body: &'a str,
    post_thumbnail: &'a str,
    post_thumbnail_alt: &'a str,
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "404: The resource you have requested could not be found.",
    )
}

async fn index(State(state): State<Arc<Vec<Post>>>) -> impl IntoResponse {
    let template: IndexTemplate = IndexTemplate {
        posts: &state.to_vec(),
    };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "try again later").into_response(),
    }
}

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate<'a> {
    posts: &'a Vec<Post>,
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
