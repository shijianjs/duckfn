import type {CodeEditor} from './codemirror';
import {mountCodeEditor} from './codemirror';
import {el} from './dom';

/**
 * The "edit the source" dialog shared by `<dfk-mermaid>` and the SQL result `svg`
 * viewer: a modal holding a plain-text CodeMirror editor and an Apply / Cancel
 * pair.
 *
 * The editor writes back through the callback {@link open} is given rather than
 * through an event, so the owner keeps its own source as the single truth — the
 * dialog never renders anything and never decides what a change means.
 *
 * The `<dialog>` is built once and reused; the editor mounts on first open and is
 * kept, because a reader who edits one diagram is likely to edit the next. The
 * dialog is shown *before* the editor mounts: CodeMirror measures its container as
 * it is constructed, and a `display: none` dialog measures to zero.
 *
 * This is browser-only code.
 */

export interface SourceDialogLabels {
  title: string;
  apply: string;
  cancel: string;
}

export class SourceDialog {
  readonly root = el('dialog', {class: 'dfk-source-dialog'});
  readonly #title = el('h2', {class: 'dfk-source-dialog-title'});
  /** Where the CodeMirror editor mounts, inside the dialog. */
  readonly #editorHost = el('div', {class: 'dfk-source-dialog-editor'});
  readonly #applyBtn = el('button', {
    class: 'dfk-source-dialog-button dfk-source-dialog-apply',
    type: 'button',
  });
  readonly #cancelBtn = el('button', {class: 'dfk-source-dialog-button', type: 'button'});

  /** The source the dialog was opened with, and the baseline for "was it edited". */
  #source = '';
  #onApply: ((value: string) => void) | null = null;
  #editor: CodeEditor | null = null;
  #editorLoading = false;

  constructor(labels: SourceDialogLabels) {
    this.#applyBtn.addEventListener('click', () => this.#apply());
    this.#cancelBtn.addEventListener('click', () => this.root.close());
    this.root.append(
      el('div', {class: 'dfk-source-dialog-body'}, (body) =>
        body.append(
          this.#title,
          this.#editorHost,
          el('div', {class: 'dfk-source-dialog-footer'}, (footer) =>
            footer.append(this.#cancelBtn, this.#applyBtn),
          ),
        ),
      ),
    );
    this.setLabels(labels);
  }

  setLabels(labels: SourceDialogLabels): void {
    this.#title.textContent = labels.title;
    this.#cancelBtn.textContent = labels.cancel;
    this.#applyBtn.textContent = labels.apply;
  }

  /**
   * Shows the dialog with `source` in the editor. `onApply` receives the edited
   * text — only when it differs from what was opened, so an unedited Apply is a
   * plain close.
   */
  open(source: string, onApply: (value: string) => void): void {
    this.#source = source;
    this.#onApply = onApply;
    this.root.showModal();
    if (this.#editor) {
      this.#editor.setValue(source);
      return;
    }
    void this.#mountEditor();
  }

  /** Closes the dialog and releases the editor. The dialog node stays in place. */
  destroy(): void {
    this.#editor?.destroy();
    this.#editor = null;
    if (this.root.open) {
      this.root.close();
    }
  }

  async #mountEditor(): Promise<void> {
    if (this.#editorLoading) {
      return;
    }
    this.#editorLoading = true;
    try {
      // No language: the dialog edits mermaid source or SVG markup, neither of
      // which has a first-party CodeMirror grammar here, and the reader is
      // touching up a figure rather than writing SQL.
      const editor = await mountCodeEditor(this.#editorHost, this.#source, () => undefined);
      if (!this.root.isConnected || !this.root.open) {
        // Closed (or detached) while the modules were loading: nothing will ever
        // dispose this editor, so dispose it here.
        editor.destroy();
        return;
      }
      editor.setWrap(true);
      this.#editor = editor;
    } catch {
      // The dialog stays open with an empty editor box; a second click retries.
    } finally {
      this.#editorLoading = false;
    }
  }

  #apply(): void {
    const edited = this.#editor?.getValue();
    this.root.close();
    if (edited !== undefined && edited !== this.#source) {
      this.#source = edited;
      this.#onApply?.(edited);
    }
  }
}
