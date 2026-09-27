/**
 * The CodeMirror 6 editor, mounted only when the reader clicks "Edit".
 *
 * Every CodeMirror module arrives through dynamic `import()` inside
 * {@link mountSqlEditor}: a page full of SQL examples pays nothing for the
 * editor until one of them is actually edited, and Docusaurus' Node prerender
 * never evaluates any of it.
 */

export interface SqlEditor {
  getValue(): string;
  setValue(value: string): void;
  destroy(): void;
}

export async function mountSqlEditor(
  container: HTMLElement,
  value: string,
  onChange: (value: string) => void,
): Promise<SqlEditor> {
  const [{basicSetup}, {sql}, {EditorView, keymap}, {defaultKeymap, historyKeymap}] =
    await Promise.all([
      import('codemirror'),
      import('@codemirror/lang-sql'),
      import('@codemirror/view'),
      import('@codemirror/commands'),
    ]);

  const view = new EditorView({
    doc: value,
    extensions: [
      basicSetup,
      sql(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChange(update.state.doc.toString());
        }
      }),
    ],
    parent: container,
  });

  return {
    getValue: () => view.state.doc.toString(),
    setValue: (next: string) =>
      view.dispatch({
        changes: {from: 0, to: view.state.doc.length, insert: next},
      }),
    destroy: () => view.destroy(),
  };
}
