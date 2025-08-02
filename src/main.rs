use std::{
    collections::{HashMap, HashSet},
    env,
    io::{self, Write},
};

use nix_index::database::Reader;
use regex::bytes::Regex;

fn export_nix_index_to_sqlite_with_transaction<F>(db_path: &str, export_logic: F)
where
    F: Fn(&mut dyn Write, Reader),
{
    let reader = Reader::open(db_path).expect("Failed to open nix-index database");
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    writeln!(handle, "PRAGMA journal_mode=WAL;").unwrap();
    writeln!(handle, "PRAGMA synchronous=OFF;").unwrap();

    writeln!(handle, "BEGIN;").unwrap();

    export_logic(&mut handle, reader);

    writeln!(handle, "COMMIT;").unwrap();
}

fn main() {
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .expect("Please provide the path to the nix-index database");
    let maybe_dump = args.next();

    if let Some(flag) = maybe_dump {
        if flag == "--dump-sqlite-fulltext-search" {
            dump_sqlite_for_fulltext_search_to_stdout(&path);
            return;
        }
        if flag == "--dump-sqlite-normalized" {
            dump_sqlite_normalized_to_stdout(&path);
            return;
        }
        if flag == "--dump-sqlite-pkgconfig-libs" {
            dump_sqlite_pkgconfig_libs_to_stdout(&path);
            return;
        }
    }

    println!(
        "No valid dump flag provided. Available flags: --dump-sqlite-fulltext-search, --dump-sqlite-normalized, --dump-sqlite-pkgconfig-libs"
    );
}

fn dump_sqlite_for_fulltext_search_to_stdout(db_path: &str) {
    export_nix_index_to_sqlite_with_transaction(db_path, |handle, reader| {
        writeln!(
            handle,
            "CREATE VIRTUAL TABLE entries USING FTS5(store_path, file_path);"
        )
        .unwrap();

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
    });
}

fn dump_sqlite_normalized_to_stdout(db_path: &str) {
    export_nix_index_to_sqlite_with_transaction(db_path, |handle, reader| {
        writeln!(
            handle,
            "CREATE TABLE packages (id INTEGER PRIMARY KEY, store_path TEXT UNIQUE);"
        )
        .unwrap();
        writeln!(handle, "CREATE TABLE files (id INTEGER PRIMARY KEY, package_id INTEGER REFERENCES packages(id), file_path TEXT);").unwrap();

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
    });
}

fn dump_sqlite_pkgconfig_libs_to_stdout(db_path: &str) {
    export_nix_index_to_sqlite_with_transaction(db_path, |handle, reader| {
        writeln!(
            handle,
            "CREATE TABLE packages (id INTEGER PRIMARY KEY, store_path TEXT UNIQUE);"
        )
        .unwrap();
        writeln!(
            handle,
            "CREATE TABLE exported_libs (id INTEGER PRIMARY KEY, package_id INTEGER REFERENCES packages(id), lib_name TEXT);"
        )
        .unwrap();

        let regex = Regex::new(r"/lib/pkgconfig/(.*)\.pc").expect("Failed to compile regex");

        let mut next_package_id = 1u64;
        let mut next_lib_id = 1u64;
        let mut package_libs: HashMap<String, HashSet<String>> = HashMap::new();

        if let Ok(iter) = reader.query(&regex).run() {
            for entry in iter {
                if let Ok((store, file)) = entry {
                    let store_str = store.as_str().replace('\'', "''");
                    let file_path = String::from_utf8_lossy(&file.path);
                    if let Some(caps) = regex.captures(file_path.as_bytes()) {
                        if let Some(lib_name) = caps.get(1) {
                            let lib_name_str =
                                String::from_utf8_lossy(lib_name.as_bytes()).replace('\'', "''");
                            package_libs
                                .entry(store_str.clone())
                                .or_default()
                                .insert(lib_name_str);
                        }
                    }
                }
            }
        }

        // Only include packages that export at least one library
        for (store_str, libs) in package_libs.iter() {
            let package_id = next_package_id;
            writeln!(
                handle,
                "INSERT INTO packages (id, store_path) VALUES ({}, '{}');",
                package_id, store_str
            )
            .unwrap();
            next_package_id += 1;

            for lib_name in libs {
                writeln!(
                    handle,
                    "INSERT INTO exported_libs (id, package_id, lib_name) VALUES ({}, {}, '{}');",
                    next_lib_id, package_id, lib_name
                )
                .unwrap();
                next_lib_id += 1;
            }
        }
    });
}
