import React from 'react';
import CodeBlock from '@theme/CodeBlock';
import styles from './styles.module.css';

interface CodeCompareProps {
  /** Caption above the left column — the raw implementation. */
  leftTitle: string;
  /** Caption above the right column — the duckfn implementation. */
  rightTitle: string;
  /** The raw implementation. Surrounding blank lines and indentation are stripped. */
  left: string;
  /** The duckfn implementation. Surrounding blank lines and indentation are stripped. */
  right: string;
  /** Prism language used for both columns. */
  language?: string;
}

/**
 * Strips the blank lines around a snippet and the indentation it was written
 * with, so the snippets can stay readable in the Markdown source.
 */
function dedent(code: string): string {
  const lines = code.replace(/^\n+/, '').replace(/\s+$/, '').split('\n');
  const indents = lines
    .filter((line) => line.trim() !== '')
    .map((line) => line.length - line.trimStart().length);
  const common = indents.length > 0 ? Math.min(...indents) : 0;
  return lines.map((line) => line.slice(common)).join('\n');
}

/**
 * Two implementations of the same function, side by side. Stacks into a single
 * column on narrow viewports.
 */
export default function CodeCompare({
  leftTitle,
  rightTitle,
  left,
  right,
  language = 'rust',
}: CodeCompareProps): React.JSX.Element {
  return (
    <div className={styles.compare}>
      <div className={styles.column}>
        <div className={styles.caption}>{leftTitle}</div>
        <CodeBlock language={language}>{dedent(left)}</CodeBlock>
      </div>
      <div className={styles.column}>
        <div className={styles.caption}>{rightTitle}</div>
        <CodeBlock language={language}>{dedent(right)}</CodeBlock>
      </div>
    </div>
  );
}
