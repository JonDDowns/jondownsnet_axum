use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use sqlx::types::time::Date;
use sqlx::FromRow;
use std::sync::Arc;

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

#[derive(Template)]
#[template(path = "blogtempl.html", escape = "none")]
pub struct PostTemplate<'a> {
    post_title: &'a str,
    post_date: String,
    post_body: &'a str,
    post_thumbnail: &'a str,
    post_thumbnail_alt: &'a str,
}

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
pub struct IndexTemplate<'a> {
    posts: &'a Vec<Post>,
}

pub async fn index(State(state): State<Arc<Vec<Post>>>) -> impl IntoResponse {
    let template: IndexTemplate = IndexTemplate {
        posts: &state.to_vec(),
    };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "try again later").into_response(),
    }
}

pub async fn post(
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
