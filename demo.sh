# 官方模板构建方式，必须在bash里面运行
make debug;
duckdb -unsigned -c "
LOAD './build/debug/extension/rusty_quack/rusty_quack.duckdb_extension';
SELECT double_it3(21);
";

# cargo-duckdb-ext-tools 构建方式，任意命令环境都行
cargo duckdb-ext build; duckdb -unsigned -c "
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
  SELECT word_count_w(sentence) FROM (
      VALUES ('hello world'), ('  padded  '), (''), (NULL)
  ) t(sentence);
  ";


cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT add_it_tuple(3,5);
  ";

cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT double_it5(3);
  ";

cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT sum_list([1,2,3,4]);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT sum_list(v) from (values ([1,2,3,4]),([1,2])) t(v);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT make_list_scalar() from (values ([1,2,3,4]),([1,2])) t(v);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT make_list_scalar(range) from range(10);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT make_list_scalar_w(range) from range(10);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT nest_list_scalar_w(range) from range(10);
  ";


cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT sum_list_w([1,2,3,4]);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT sum_list_nest([[1,2],[3,4]]);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT sum_list_nest([[1,2],[3,null,4],null]);";

cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT sum_list_nest(v) from (values ([[1,2],[3,null,4],null]),([[1],[3,null]])) t(v);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT make_pair('hello', 42);
  ";
cargo duckdb-ext build; duckdb -unsigned -c "
  LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT make_kv_map('hello', 42);
  ";

cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  from count_down(12);";

cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT struct_scalar_w({hello_count:15});";

cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT struct_nest_scalar_w({struct:{hello_count:15},list:[1,null,2]});";

cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT struct_nest_output_scalar_w(range::int) from range(9);";


cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT agg_list_w(range) from range(9) group by range;";


cargo duckdb-ext build; duckdb -unsigned -c "LOAD './target/debug/rusty_quack.duckdb_extension';
  SELECT range % 3 as g,agg_list_w(range) from range(9) group by g;";

