# Webserivce to search in the nix-index database

## Directly using the nix-index-code to search the db

Caveats: The code is structured in such a way, that each search opens and decompresses the whole database. That is surprisingly inefficient, about 650ms on my machine.

```fish
❯ cargo run --release -- /Users/dwt/.cache/nix-index/files
```

## Converting the nix-index database to a sqlite database - stupid full text index

This works surprisingly well. It is very space inefficient, in that it blows up the database from 48mb to 3.7gb. It still is very fast, around 6 ms per search, so already about a 100x improvement.

```fish
❯ rm db.sqlite ; cargo run --release -- /Users/dwt/.cache/nix-index/files --dump-sqlite | sqlite3 db.sqlite

❯ time sqlite3 db-big.sqlite 'select package_name, nix_path from entries  where nix_path match "libmagic pc" limit 100;' -csv
-- Loading resources from /Users/dwt/.sqliterc
package_name,nix_path
/nix/store/qsizaj9x3hwjh6y0qywqh604b5r0762i-file-5.45-dev,/lib/pkgconfig/libmagic.pc

________________________________________________________
Executed in    6.62 millis    fish           external
   usr time    1.92 millis  224.00 micros    1.69 millis
   sys time    2.69 millis  720.00 micros    1.97 millis
```

## Converting the nix-index databse to sqlite with more sophistication

Idea is to have one table for the package, and one for the files, and each file links to the package. That still allows to search for substrings in the files table, though potentially with a full table scan?

Without any index, that table is signifficantly smaller, at about 1.2gb. The table scan on the files table for like queries takes about 1.5s, so signifficantly longer than the fulltext or nix-index search. Indexes don't really help here, as they do not support full like queries, only prefix queries.

## Maybe best: just go from the database directly to a mapping of libraries provided by packages
