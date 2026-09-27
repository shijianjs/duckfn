import type {Plugin} from 'unified';

/**
 * Replaces a build-time version placeholder in the docs tree.
 *
 * Written as a plain text substitution so it can live in a shared package: the
 * version value is passed in by the consuming site (it is the one thing that is
 * *not* shared), and only "pure text" carriers are touched —
 * `text` / `inlineCode` / `code` nodes. MDX expression nodes (`{expr}`) and ESM
 * nodes are deliberately left alone, so this never interferes with MDX's own
 * evaluation.
 *
 * This is Node-side build code: it must not touch `window` / `document`.
 */
export const DEFAULT_VERSION_PLACEHOLDER = '{{DUCKFN_VERSION}}';

export interface VersionPlaceholderOptions {
  /** The real version string to substitute in, e.g. `0.0.13`. */
  version: string;
  /** Override the token if a site uses a different one. */
  placeholder?: string;
}

const TEXT_NODE_TYPES = new Set(['text', 'inlineCode', 'code']);

interface TextLikeNode {
  type: string;
  value?: unknown;
  children?: TextLikeNode[];
}

export const remarkVersionPlaceholder: Plugin<[VersionPlaceholderOptions]> =
  ({version, placeholder = DEFAULT_VERSION_PLACEHOLDER}) =>
  (tree) => {
    const walk = (node: unknown): void => {
      if (typeof node !== 'object' || node === null) {
        return;
      }

      const candidate = node as TextLikeNode;
      const value = candidate.value;

      if (
        typeof candidate.type === 'string' &&
        typeof value === 'string' &&
        TEXT_NODE_TYPES.has(candidate.type) &&
        value.includes(placeholder)
      ) {
        // Exact-token replacement only — no general `{{…}}` parsing — so other
        // content is left untouched.
        candidate.value = value.split(placeholder).join(version);
      }

      if (Array.isArray(candidate.children)) {
        candidate.children.forEach(walk);
      }
    };

    walk(tree);
  };
