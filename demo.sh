# 官方模板构建方式，必须在bash里面运行
make debug;
duckdb -unsigned -c "
LOAD './build/debug/extension/rusty_quack/rusty_quack.duckdb_extension';
SELECT double_it3(21);
";

# cargo-duckdb-ext-tools 构建方式，任意命令环境都行
cargo duckdb-ext build;
duckdb -unsigned -c "
LOAD './target/debug/rusty_quack.duckdb_extension';
SELECT double_it5(21);
";

cargo duckdb-ext build; duckdb -unsigned -c "
LOAD './target/debug/rusty_quack.duckdb_extension';
SELECT first_word5(sentence) FROM (
    VALUES ('hello world'), ('  padded  '), (''), (NULL)
) t(sentence);
";
cargo duckdb-ext build; duckdb -unsigned -c "
LOAD './target/debug/rusty_quack.duckdb_extension';
SELECT word_count(sentence) FROM (
    VALUES ('hello world'), ('  padded  '), (''), (NULL)
) t(sentence);
";


cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT add_it_tuple(3,5);
  ";
