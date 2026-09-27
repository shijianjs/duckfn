/**
 * The CodeMirror 6 editor that *is* the code view of a runnable SQL block.
 *
 * Every CodeMirror module arrives through dynamic `import()` inside
 * {@link mountSqlEditor}: a page full of SQL examples pays nothing for the
 * editor on its critical path, and Docusaurus' Node prerender never evaluates
 * any of it.
 */

export interface SqlEditor {
  getValue(): string;
  setValue(value: string): void;
  /** Soft-wraps long lines, or stops wrapping them (the block's wrap toggle). */
  setWrap(wrapped: boolean): void;
  destroy(): void;
}

export async function mountSqlEditor(
  container: HTMLElement,
  value: string,
  onChange: (value: string) => void,
): Promise<SqlEditor> {
  const [
    {basicSetup},
    {sql},
    {EditorView, keymap},
    {defaultKeymap, historyKeymap},
    {Compartment},
  ] = await Promise.all([
    import('codemirror'),
    import('@codemirror/lang-sql'),
    import('@codemirror/view'),
    import('@codemirror/commands'),
    import('@codemirror/state'),
  ]);

  // Wrapping is toggled from the outside, and reconfiguring it must not disturb
  // the document or the undo history — that is exactly what a compartment is
  // for, so the extension is swapped in place rather than rebuilt.
  const wrap = new Compartment();

  const view = new EditorView({
    doc: value,
    extensions: [
      basicSetup,
      sql(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      wrap.of([]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChange(update.state.doc.toString());
        }
      }),
    ],
    parent: container,
    // `root` is left to CodeMirror's own `getRoot(container)`. The container sits
    // in `<dfk-sql>`'s shadow root, so style-mod mounts the base theme into that
    // same shadow root — exactly where the `.cm-*` rules are needed. Pinning it
    // to `document` would put them outside the editor's tree instead, where a
    // shadow boundary stops them.
  });

  return {
    getValue: () => view.state.doc.toString(),
    setValue: (next: string) =>
      view.dispatch({
        changes: {from: 0, to: view.state.doc.length, insert: next},
      }),
    setWrap: (wrapped: boolean) =>
      view.dispatch({
        effects: wrap.reconfigure(wrapped ? EditorView.lineWrapping : []),
      }),
    destroy: () => view.destroy(),
  };
}
