-- duck_sql_macro_files! 的第 1 个文件：一份脚本注册多个标量宏。
CREATE OR REPLACE MACRO dfn_macro_files_add(a, b) AS (a + b);

CREATE OR REPLACE MACRO dfn_macro_files_mul(a, b) AS (a * b);
