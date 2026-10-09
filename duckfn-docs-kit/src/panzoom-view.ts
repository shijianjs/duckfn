import type {PanzoomObject} from '@panzoom/panzoom';

/**
 * The zoom / pan behaviour behind a rendered figure, shared by `<dfk-mermaid>`
 * and the SQL result `svg` viewer.
 *
 * It owns no structure: the caller builds the viewport and the content box (each
 * with its own class names, so each consumer's stylesheet can reach them) and
 * hands both over. What lives here is the interaction contract — when panzoom may
 * take the gesture, and how the zoom state is reflected back onto the viewport.
 *
 * **Zoom is opt-in and reversible.** panzoom starts disabled and only
 * {@link setActive} turns it on, which is what lets an inline figure leave the
 * page's scrolling alone: a wheel over a figure that is merely displayed is the
 * browser's, and only a figure the reader has opened moves (the SQL result's
 * fullscreen, a diagram's own). Deactivating resets the view to fit, so the next
 * activation starts from the whole figure.
 *
 * **While it is active the figure is a viewport, not a picture.** The wheel
 * zooms both ways — including below fit, so a figure can be shrunk — a drag
 * always pans, and the pointer is a grab hand throughout. Nothing tries to keep
 * the diagram's labels selectable while zoom is on: a drag that panned anyway
 * would silently steal the selection, which made the whole gesture ambiguous.
 * Inline, where panzoom is off, the labels are ordinary text again and a drag
 * selects them.
 *
 * This is browser-only code: `@panzoom/panzoom` is reached through a dynamic
 * `import()`, so a page whose figures are never zoomed never loads it.
 */

/**
 * The zoom range, in multiples of fit (scale 1, which is where a figure opens and
 * where "Reset zoom" returns it). `MIN_SCALE` below 1 is what lets a reader shrink
 * a figure that fills the viewport; the ceiling is high enough to inspect a dense
 * scatter plot.
 */
const MIN_SCALE = 0.25;
const MAX_SCALE = 8;
const ZOOM_STEP = 0.25;

/** The pan/zoom library, loaded once per page on first use. */
let panzoomModule: Promise<typeof import('@panzoom/panzoom')['default']> | null = null;

function loadPanzoom(): Promise<typeof import('@panzoom/panzoom')['default']> {
  panzoomModule ??= import('@panzoom/panzoom').then((module) => module.default);
  return panzoomModule;
}

export class PanZoomView {
  readonly #viewport: HTMLElement;
  readonly #content: HTMLElement;
  #panzoom: PanzoomObject | null = null;
  #loading = false;
  #active = false;
  /** Whether the viewport's wheel listener is currently attached. */
  #wheelBound = false;

  readonly #onWheel = (event: WheelEvent): void => {
    this.#panzoom?.zoomWithWheel(event);
  };
  readonly #onStart = (): void => {
    this.#viewport.classList.add('dfk-panzoom-grabbing');
  };
  readonly #onEnd = (): void => {
    this.#viewport.classList.remove('dfk-panzoom-grabbing');
  };

  constructor(viewport: HTMLElement, content: HTMLElement) {
    this.#viewport = viewport;
    this.#content = content;
    // panzoom reports the start and end of a gesture as DOM `CustomEvent`s
    // dispatched on the element it transforms (`@panzoom/panzoom` v4 has no `on()`
    // API), so they are listened for here, on the content box — which also means
    // they need no teardown, and that re-creating the panzoom instance after a
    // reconnect cannot double them.
    content.addEventListener('panzoomstart', this.#onStart);
    content.addEventListener('panzoomend', this.#onEnd);
  }

  /** Replaces the figure on screen and returns the view to fit. */
  setContent(node: Node | null): void {
    this.#content.replaceChildren(...(node ? [node] : []));
    this.reset();
  }

  /**
   * Turns wheel zoom and dragging on or off. Activating loads panzoom on first
   * use (a figure nobody zooms never pays for the chunk); deactivating leaves the
   * view at fit, so the figure a reader returns to is the whole one.
   */
  setActive(active: boolean): void {
    if (active === this.#active) {
      return;
    }
    this.#active = active;
    // The grab cursor follows "this figure is a viewport now", not the zoom
    // level: inside one a drag always pans and the wheel always zooms, so the
    // hand is honest from the first moment — and the I-beam stops promising a
    // text selection that the drag would not deliver.
    this.#viewport.classList.toggle('dfk-panzoom-active', active);
    if (active) {
      void this.#ensurePanzoom();
    } else {
      this.reset();
      this.#setEnabled(false);
      this.#unbindWheel();
    }
  }

  reset(): void {
    this.#panzoom?.reset({
      // Zooming back to fit is a transition the reader did not ask to skip, but
      // one they may have asked not to have.
      animate: !prefersReducedMotion(),
    });
  }

  /** Releases the panzoom instance and returns the viewport to its idle state. */
  destroy(): void {
    this.#active = false;
    this.#unbindWheel();
    this.#panzoom?.destroy();
    this.#panzoom = null;
    this.#viewport.classList.remove('dfk-panzoom-active', 'dfk-panzoom-grabbing');
  }

  /**
   * Binds panzoom to the content box. `@panzoom/panzoom` is an enhancement, not a
   * requirement: if its chunk never arrives, the figure still renders, only wheel
   * zoom and dragging stay inert.
   *
   * Three settings carry the interaction contract:
   *
   * - `panOnlyWhenZoomed: false` — panning works at fit too, which is what a
   *   fullscreen figure should do; `touchAction: 'pan-y'` keeps vertical page
   *   scrolling the browser's while pinch and horizontal drags go to the figure.
   * - `handleStartEvent` — panzoom's own default takes every pointerdown
   *   (`preventDefault` + `stopPropagation`), which is what stops a drag from
   *   selecting text instead of panning. It is applied here only while the view is
   *   active: inline, the guard leaves the gesture alone so a diagram's labels
   *   stay selectable and a drag still selects them.
   * - no `cursor` option — panzoom would then put `grab` on the element for good,
   *   inline included, over text that is perfectly selectable. The cursor comes
   *   from each consumer's stylesheet, driven by the active state instead.
   */
  async #ensurePanzoom(): Promise<void> {
    if (this.#panzoom !== null) {
      this.#setEnabled(true);
      this.#bindWheel();
      return;
    }
    if (this.#loading) {
      return;
    }
    this.#loading = true;
    try {
      const Panzoom = await loadPanzoom();
      // Deactivated while the chunk was in flight: nothing to build, and the next
      // activation starts from here again.
      if (!this.#active || this.#panzoom !== null) {
        return;
      }
      this.#panzoom = Panzoom(this.#content, {
        maxScale: MAX_SCALE,
        minScale: MIN_SCALE,
        step: ZOOM_STEP,
        panOnlyWhenZoomed: false,
        touchAction: 'pan-y',
        // panzoom's own default is `move`, written inline on the element — the
        // cursor has to stay a CSS decision (see the consumers' stylesheets), so
        // it is switched off here.
        cursor: '',
        disablePan: false,
        disableZoom: false,
        handleStartEvent: (event) => {
          if (!this.#active) {
            return;
          }
          event.preventDefault();
          event.stopPropagation();
        },
      });
      this.#clearPanzoomStyles();
      this.#bindWheel();
    } catch {
      // Nothing to report: the figure is already usable without pan/zoom.
    } finally {
      this.#loading = false;
    }
  }

  #setEnabled(enabled: boolean): void {
    if (this.#panzoom === null) {
      return;
    }
    this.#panzoom.setOptions({disablePan: !enabled, disableZoom: !enabled});
    this.#clearPanzoomStyles();
  }

  /**
   * panzoom forces `user-select: none` inline on the element *and its parent*,
   * with no option to prevent it — that alone made every label in every diagram
   * unselectable. It exists to stop a drag from selecting while panning, which
   * `handleStartEvent` already covers: whenever a pan will happen, the gesture is
   * taken with `preventDefault()` before the browser can start a selection.
   * panzoom writes these styles when the instance is created and again on every
   * `setOptions`, so they are cleared after each.
   */
  #clearPanzoomStyles(): void {
    this.#content.style.userSelect = '';
    this.#viewport.style.userSelect = '';
  }

  #bindWheel(): void {
    if (this.#wheelBound) {
      return;
    }
    this.#wheelBound = true;
    // `passive: false` because `zoomWithWheel()` calls `preventDefault()` — a
    // passive listener could not stop the page from scrolling underneath.
    this.#viewport.addEventListener('wheel', this.#onWheel, {passive: false});
  }

  #unbindWheel(): void {
    if (!this.#wheelBound) {
      return;
    }
    this.#wheelBound = false;
    this.#viewport.removeEventListener('wheel', this.#onWheel);
  }
}

function prefersReducedMotion(): boolean {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}
