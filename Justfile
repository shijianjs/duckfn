#set windows-shell := ["pwsh.exe", "-NoLogo","-Command"]

set windows-shell := ["C:\\Program Files\\Git\\bin\\bash.exe", "-c"]

extension := "./target/debug/rusty_quack.duckdb_extension"

release:
  cargo build --release    


ext_build:
    cargo duckdb-ext build

duckdb_ext sql: ext_build
    duckdb -unsigned -c "LOAD '{{extension}}'; {{sql}}"

duckdb_ext_debug sql: ext_build
    duckdb -unsigned -cmd "LOAD '{{extension}}'; {{sql}}"

test:
    make debug test