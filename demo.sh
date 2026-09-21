# 官方模板构建方式，必须在bash里面运行
make debug;
duckdb -unsigned -c "
LOAD './build/debug/extension/rusty_quack/rusty_quack.duckdb_extension';
";

# cargo-duckdb-ext-tools 构建方式，任意命令环境都行

just sql "  SELECT double_it5(21);";


just sql "  SELECT first_word_tuple(sentence) FROM (
      VALUES ('hello world'), ('  padded  '), (''), (NULL)
  ) t(sentence);";
just sql "  SELECT word_count_w(sentence) FROM (
      VALUES ('hello world'), ('  padded  '), (''), (NULL)
  ) t(sentence);";
just sql "  SELECT word_count_m(sentence) FROM (
      VALUES ('hello world'), ('  padded  '), (''), (NULL)
  ) t(sentence);";


just sql "  SELECT add_it_tuple(3,5);  ";

just sql "  SELECT double_it5(3);  ";

just sql "  SELECT make_list_scalar_w(range) from range(10);";
just sql "  SELECT nest_list_scalar_w(range) from range(10);  ";


just sql "  SELECT nest_vec_no_null_scalar_w(range) from range(10);  ";


just sql "  SELECT sum_list_w([1,2,3,4]);";
just sql "  SELECT sum_list_nest([[1,2],[3,4]]);";
just sql "  SELECT sum_list_nest([[1,2],[3,null,4],null]);";

just sql "  SELECT sum_list_nest(v) from (values ([[1,2],[3,null,4],null]),([[1],[3,null]])) t(v);";

just sql "  SELECT struct_scalar_w({hello_count:15});";

just sql "  SELECT struct_nest_scalar_w({structf:{hello_count:15},list:[1,null,2]});";

just sql "  SELECT struct_nest_output_scalar_w(range::int) from range(9);";

just sql "  SELECT agg_list_w(range) from range(9) group by range;";


just sql "  SELECT range % 3 as g,agg_list_w(range) from range(9) group by g;";

just sql "  SELECT error_scalar_demo(3);";
just sql "  SELECT error_scalar_demo(10);";
just sql "  SELECT error_scalar_demo(20);";
just sql "  SELECT error_scalar_demo(30);";


just sql "  from count_down_m_simple(start=12);";
just sql "  SELECT create_map_demo(range) from range(10);";
just sql "  SELECT input_map_demo(MAP {'key1': [10], 'key2': [20,5], 'key3': null});";
just sql "  SELECT input_map_notnull_demo(MAP {'key1': [10], 'key2': [20], 'key3': []});";
just sql "  from bind_map_demo(MAP {'key1': [10], 'key2': [20,5], 'key3': null});";

just sql "  SELECT input_array_demo(a) from (values (ARRAY [1, 2]),(ARRAY [4, null])) t(a);";
just sql "  SELECT input_array_notnull_demo(a) from (values (ARRAY [1, 2]),(ARRAY [4, null])) t(a);";

just sql "  SELECT clamp(range,4, 7) from range(9);";

# 嵌套父struct为null写入错误: 已修复
just sql "  SELECT (dfn_echo_struct_simple(x)).id FROM (VALUES (NULL::STRUCT(id INTEGER, name VARCHAR)), ({'id': 1, 'name': 'a'})) t(x);";
# 单行/常量输入 + 嵌套 NULL struct 字段：写侧子字段未置 NULL 曾崩溃 0xC0000005，已修复（a6a3214）
just sql "SELECT dfn_echo_struct_nested_only({'id': 1, 'inner': NULL::STRUCT(key VARCHAR, value INTEGER)});";
# struct列表字面量子字段全NULL读取异常
just sql "SELECT dfn_echo_struct_list_nullable([NULL, NULL, NULL, NULL]::STRUCT(id INTEGER, name VARCHAR)[]);";
# 异常输出： [NULL, {'id': 657, 'name': ''}, {'id': 1821221984, 'name': ''}, {'id': 657, 'name': ''}]
# 声明类型就没问题：
just sql "SELECT dfn_echo_struct_list_nullable( [ NULL::STRUCT(id INTEGER, name VARCHAR), NULL::STRUCT(id INTEGER, name VARCHAR) ] )";
#  [NULL, NULL]
