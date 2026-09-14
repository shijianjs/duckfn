-- duck_sql_macro_files! 的第 2 个文件：脚本里可以有表宏。
CREATE OR REPLACE MACRO dfn_macro_files_gen(n) AS TABLE SELECT * FROM range(n);
