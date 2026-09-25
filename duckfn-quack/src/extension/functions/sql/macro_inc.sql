-- 这个脚本在编译期被 include_str! 内联进扩展（见 ../sql_macro.rs），
-- 扩展初始化时整段由 duckdb_query 执行。
-- 一条脚本可以注册多个宏：语句之间用分号分隔，注释用 --。
CREATE OR REPLACE MACRO dfn_macro_inc_add(a, b) AS (a + b);

CREATE OR REPLACE MACRO dfn_macro_inc_triple(x) AS (x * 3);

CREATE OR REPLACE MACRO dfn_macro_inc_gen(n) AS TABLE SELECT * FROM range(n);
