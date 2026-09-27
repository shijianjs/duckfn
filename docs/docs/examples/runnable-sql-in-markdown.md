---
title: Runnable SQL in Markdown
sidebar_position: 4
description: The runnable SQL block also works in a plain .md file — the syntax needs no MDX.
---

# Runnable SQL in Markdown

This page is a plain `.md` file, not `.mdx`. A runnable block needs no MDX
feature: the JSON metastring is read by a remark plugin during the build, which
rewrites the block into the custom element *before* either format is compiled,
and Docusaurus passes that result through unchanged for `.md`.

Everything on this page therefore behaves exactly like its `.mdx` counterpart —
see [Runnable SQL examples](./runnable-sql.mdx) for the full syntax and the config
reference.

## A table

```sql {"type":"duckfn","show":"table"}
SELECT * FROM range(5) AS t(n);
```

## A scalar

`show` may be omitted; a single scalar result renders as text.

```sql {"type":"duckfn"}
SELECT 40 + 2 AS answer;
```