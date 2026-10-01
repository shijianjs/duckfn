/**
 * The CodeMirror 6 editor behind the kit's editable content: the code view of a
 * runnable SQL block (`<dfk-sql>`) and the diagram-source dialog of a
 * `<dfk-mermaid>`. One implementation, two languages — the SQL block asks for
 * highlighting, the diagram editor takes the plain-text default.
 *
 * Every CodeMirror module arrives through a dynamic `import()` inside
 * {@link mountCodeEditor}: a page full of blocks pays nothing for the editor on
 * its critical path, and Docusaurus' Node prerender never evaluates any of it.
 *
 * This is browser-only code: `document` is touched only through the container
 * the caller hands over, and everything else is behind the `import()`s.
 */

export interface CodeEditor {
  getValue(): string;
  setValue(value: string): void;
  /** Soft-wraps long lines, or stops wrapping them (the block's wrap toggle). */
  setWrap(wrapped: boolean): void;
  destroy(): void;
}

export interface CodeEditorOptions {
  /**
   * Highlight the document as SQL. Omitted, the editor is plain text — which is
   * what a mermaid diagram gets: there is no first-party CodeMirror language for
   * it, and a docs reader is editing prose-shaped source, not writing SQL.
   */
  language?: 'sql';
}

export async function mountCodeEditor(
  container: HTMLElement,
  value: string,
  onChange: (value: string) => void,
  options: CodeEditorOptions = {},
): Promise<CodeEditor> {
  const [{basicSetup}, {EditorView, keymap}, {defaultKeymap, historyKeymap}, {Compartment}] =
    await Promise.all([
      import('codemirror'),
      import('@codemirror/view'),
      import('@codemirror/commands'),
      import('@codemirror/state'),
    ]);
  // Loaded only for the blocks that ask for it, so a diagram editor does not pull
  // the SQL grammar (and its lezer parsers) into the page.
  const language =
    options.language === 'sql' ? [(await import('@codemirror/lang-sql')).sql()] : [];

  // Wrapping is toggled from the outside, and reconfiguring it must not disturb
  // the document or the undo history — that is exactly what a compartment is
  // for, so the extension is swapped in place rather than rebuilt.
  const wrap = new Compartment();

  const view = new EditorView({
    doc: value,
    extensions: [
      basicSetup,
      ...language,
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
    // in the component's shadow root, so style-mod mounts the base theme into
    // that same shadow root — exactly where the `.cm-*` rules are needed.
    // Pinning it to `document` would put them outside the editor's tree instead,
    // where a shadow boundary stops them.
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