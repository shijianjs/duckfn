import type {DetailedHTMLProps, HTMLAttributes} from 'react';

/**
 * `<iconify-icon>` is the official Iconify web component, registered as a side
 * effect by `registerDfkElements()`. The package ships no JSX types of its own,
 * and `createElement('iconify-icon', …)` was the workaround that kept the tag
 * out of the JSX namespace — but that also kept it out of the JSX, at the cost
 * of readability. One declaration here is enough to write it as a tag like
 * everything else around it.
 */
declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'iconify-icon': DetailedHTMLProps<
        HTMLAttributes<HTMLElement>,
        HTMLElement
      > & {
        /** Iconify name, e.g. `lucide:arrow-right`; the element's whole point. */
        icon: string;
      };
    }
  }
}
