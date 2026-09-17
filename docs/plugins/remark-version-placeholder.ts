import type {Plugin} from 'unified';
import {DUCKFN_VERSION} from '../duckfn-version';

// 文档里写这个占位符，构建时会被替换成 DUCKFN_VERSION。
// 请写在围栏代码块或行内代码里（那里的大括号是原文，MDX 不会当成表达式）。
export const VERSION_PLACEHOLDER = '{{DUCKFN_VERSION}}';

// 只处理「纯文本载体」节点：
//   text       普通正文
//   inlineCode `行内代码`
//   code       ```围栏代码块
// MDX 的表达式节点（mdxTextExpression / mdxFlowExpression，也就是 `{expr}`）与
// ESM 节点不在其中，因此这里不会干扰 MDX 自身的求值行为。
const TEXT_NODE_TYPES = new Set(['text', 'inlineCode', 'code']);

interface TextLikeNode {
  type: string;
  value?: unknown;
  children?: TextLikeNode[];
}

/**
 * 把占位符替换成真实版本号。
 *
 * 这是构建期的一次性文本替换：只替换精确的 `{{DUCKFN_VERSION}}`，
 * 不做任何通用的 `{{…}}` 解析，所以对其它内容零影响。
 */
const remarkVersionPlaceholder: Plugin = () => (tree) => {
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
            value.includes(VERSION_PLACEHOLDER)
        ) {
            candidate.value = value.split(VERSION_PLACEHOLDER).join(DUCKFN_VERSION);
        }

        if (Array.isArray(candidate.children)) {
            candidate.children.forEach(walk);
        }
    };

    walk(tree);
};

export default remarkVersionPlaceholder;
