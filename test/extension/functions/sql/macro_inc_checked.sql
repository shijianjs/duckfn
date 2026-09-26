-- 由 DuckResult<&'static str> 路径注册：脚本内容同样在编译期内联。
-- 与 macro_inc.sql 分成两个文件，用来覆盖两条字符串返回路径。
CREATE OR REPLACE MACRO dfn_macro_inc_negate(x) AS (-x);
