use std::{collections::HashMap, env, sync::Arc};

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
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .expect("Please provide the path to the nix-index database");
    let maybe_dump = args.next();

    // dbg!(&path, &maybe_dump);
    if let Some(flag) = maybe_dump {
        if flag == "--dump-sqlite-fulltext-search" {
            dump_sqlite_for_fulltext_search_to_stdout(&path);
            return;
        }
        if flag == "--dump-sqlite-normalized" {
            dump_sqlite_normalized_to_stdout(&path);
            return;
        }
    }

    let db = Arc::new(path);
    let app = Router::new().route("/query", get(query)).with_state(db);

    let listener = tokio::net::TcpListener::bind("127.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn dump_sqlite_for_fulltext_search_to_stdout(db_path: &str) {
    use std::io::{self, Write};
    let reader = Reader::open(db_path).expect("Failed to open nix-index database");
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Emit WAL and synchronous pragmas for best performance
    writeln!(handle, "PRAGMA journal_mode=WAL;").unwrap();
    writeln!(handle, "PRAGMA synchronous=OFF;").unwrap();

    // Write the CREATE statement first
    writeln!(
        handle,
        "CREATE VIRTUAL TABLE entries USING FTS5(store_path, file_path);"
    )
    .unwrap();

    writeln!(handle, "BEGIN;").unwrap();

    let regex = Regex::new(".*").expect("Failed to compile regex");
    if let Ok(iter) = reader.query(&regex).run() {
        for entry in iter {
            if let Ok((store, file)) = entry {
                let package_name = store.as_str().replace('\'', "''");
                let nix_path = String::from_utf8_lossy(&file.path).replace('\'', "''");
                writeln!(
                    handle,
                    "INSERT INTO entries (store_path, file_path) VALUES ('{}', '{}');",
                    package_name, nix_path
                )
                .unwrap();
            }
        }
    }
    writeln!(handle, "COMMIT;").unwrap();
}

fn dump_sqlite_normalized_to_stdout(db_path: &str) {
    use std::collections::HashMap;
    use std::io::{self, Write};
    let reader = Reader::open(db_path).expect("Failed to open nix-index database");
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    writeln!(handle, "PRAGMA journal_mode=WAL;").unwrap();
    writeln!(handle, "PRAGMA synchronous=OFF;").unwrap();

    writeln!(
        handle,
        "CREATE TABLE packages (id INTEGER PRIMARY KEY, store_path TEXT UNIQUE);"
    )
    .unwrap();
    writeln!(handle, "CREATE TABLE files (id INTEGER PRIMARY KEY, package_id INTEGER REFERENCES packages(id), file_path TEXT);").unwrap();

    writeln!(handle, "BEGIN;").unwrap();

    let regex = Regex::new("").expect("Failed to compile regex");
    let mut package_ids = HashMap::new();
    let mut next_package_id = 1u64;
    let mut next_file_id = 1u64;

    if let Ok(iter) = reader.query(&regex).run() {
        for entry in iter {
            if let Ok((store, file)) = entry {
                let store_str = store.as_str().replace('\'', "''");
                let file_path = String::from_utf8_lossy(&file.path).replace('\'', "''");
                let package_id = *package_ids.entry(store_str.clone()).or_insert_with(|| {
                    let id = next_package_id;
                    writeln!(
                        handle,
                        "INSERT INTO packages (id, store_path) VALUES ({}, '{}');",
                        id, store_str
                    )
                    .unwrap();
                    next_package_id += 1;
                    id
                });
                writeln!(
                    handle,
                    "INSERT INTO files (id, package_id, file_path) VALUES ({}, {}, '{}');",
                    next_file_id, package_id, file_path
                )
                .unwrap();
                next_file_id += 1;
            }
        }
    }

    writeln!(handle, "COMMIT;").unwrap();
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
