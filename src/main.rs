use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use nix_index::database::Reader;
use regex::bytes::Regex;
use serde::Serialize;

#[tokio::main]
async fn main() {
    // initialize tracing
    // tracing_subscriber::fmt::init();

    let path = std::env::args()
        .nth(1)
        .expect("Please provide the path to the nix-index database")
        .to_string();
    let db = Arc::new(path);

    // build our application with a route
    let app = Router::new().route("/query", get(query)).with_state(db);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("127.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn query(
    State(db_path): State<Arc<String>>,
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<SearchResult>) {
    let query = params["query"].clone();

    let reader = Reader::open(db_path.as_str()).expect("Failed to open nix-index database");

    let regex = Regex::new(&query).expect("Failed to compile regex");

    // Assuming `query` is a method on `Reader` that takes a `Regex`
    let result = reader
        .query(&regex)
        .run()
        .unwrap()
        .take(10)
        .map(|r| {
            let (store, file) = r.unwrap();
            (
                store.as_str().to_string(),
                String::from_utf8_lossy(&file.path).to_string(),
            )
        })
        .collect::<Vec<_>>();

    let search_result = SearchResult {
        query,
        search_results: result,
    };
    (StatusCode::OK, Json(search_result))
}

#[derive(Serialize)]
struct SearchResult {
    query: String,
    search_results: Vec<(String, String)>,
}
