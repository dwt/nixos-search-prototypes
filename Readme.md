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

Now available as `--dump-sqlite-pkgconfig-libs`. Only 430k. Test with

```fish
❯ sqlite3 db-libs.sqlite --csv 'select store_path, lib_name from exported_libs join packages on packages.id=exported_libs.package_id limit 100'
```

This turns up quite a few 'libraries' which do not look like what I expected. Examples:

```fish
❯ sqlite3 db-libs.sqlite --csv 'select store_path, lib_name from exported_libs join packages on packages.id=exported_libs.package_id where lib_name not like "lib%" limit 10 offset 100'
-- Loading resources from /Users/dwt/.sqliterc
store_path,lib_name
/nix/store/fsjq34k80c1wl9dyjnri4w24wpfg33c5-python3-3.12.7-env,pygobject-3.0
/nix/store/lwjj5d52yb2g40nwn17cgy29l5g15p7d-notcurses-3.0.11-dev,notcurses-ffi
/nix/store/lwjj5d52yb2g40nwn17cgy29l5g15p7d-notcurses-3.0.11-dev,notcurses-core
/nix/store/lwjj5d52yb2g40nwn17cgy29l5g15p7d-notcurses-3.0.11-dev,notcurses++
/nix/store/lwjj5d52yb2g40nwn17cgy29l5g15p7d-notcurses-3.0.11-dev,notcurses
/nix/store/lkyy7z8ixa1cfgw8z9647815285mnpz8-geoclue-2.7.2-dev,geoclue-2.0
/nix/store/p2g3mrxzlrx5xh4yjxidygbphj71m5j6-icu4c-60.2-dev,icu-io
/nix/store/p2g3mrxzlrx5xh4yjxidygbphj71m5j6-icu4c-60.2-dev,icu-uc
/nix/store/p2g3mrxzlrx5xh4yjxidygbphj71m5j6-icu4c-60.2-dev,icu-i18n
/nix/store/91q4c01pvrhrjj0yjii5xvi2zdpprnzk-ogre-14.3.2,OGRE-Overlay
```
