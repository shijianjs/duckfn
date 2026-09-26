import {createElement} from 'react';
import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import Translate, {translate} from '@docusaurus/Translate';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import CodeBlock from '@theme/CodeBlock';
import Heading from '@theme/Heading';
import Layout from '@theme/Layout';
import {
  registerDfkElements,
  type FeaturesData,
  type HeroData,
  type NextStepsData,
} from 'duckfn-docs-kit';

import styles from './index.module.css';

// Defining the `dfk-*` custom elements is a one-time side effect (it also
// registers the official `<iconify-icon>` element). Idempotent, and a no-op
// during Docusaurus' Node prerender pass.
registerDfkElements();

/**
 * The landing page: hero, features, a Rust/SQL showcase and the "where next"
 * cards.
 *
 * The hero, the feature grid and the next-step cards are `dfk-*` web components
 * from duckfn-docs-kit, so the whole landing layout is reusable by other
 * extension docs sites. A custom element cannot render React's `<Translate>`,
 * so the copy is resolved with the imperative `translate()` API into plain
 * strings for the active locale and handed to the components through their
 * `data` property; the strings still live in `i18n/zh-Hans/code.json` under the
 * same `homepage.*` keys. The code showcase stays here because it needs the
 * theme's `CodeBlock`.
 *
 * Known trade-off (accepted): the `dfk-*` sections render client-side, so their
 * prerendered HTML is empty until hydration — the same behaviour as the `<Icon>`
 * glyphs.
 */

const GITHUB_URL = 'https://github.com/shijianjs/duckfn';

/**
 * Renders a `dfk-*` custom element and hands its content over through the
 * `data` **property**.
 *
 * React's SSR/hydration path only reconciles string/number props onto custom
 * elements — an object like our `data` payload is never serialised into the
 * prerendered HTML, so hydration leaves the property untouched and the element
 * renders nothing. A callback ref is the reliable channel: React calls it with
 * the live node after mount, where we can assign the property directly. The
 * element's own `data` setter then builds the subtree whether this runs before
 * or after `connectedCallback`.
 */
function dfk<TData>(
  tag: 'dfk-hero' | 'dfk-features' | 'dfk-next-steps',
  data: TData,
): ReactNode {
  return createElement(tag, {
    ref: (node: HTMLElement | null) => {
      if (node) {
        (node as unknown as {data: TData}).data = data;
      }
    },
  });
}

/**
 * Kept out of the JSX below on purpose: a template literal written inline would
 * carry the JSX indentation into the rendered code block. `use` is wrapped the
 * way rustfmt wraps it so the longest line still fits the showcase column.
 */
const RUST_SAMPLE = `use duckfn::{
    duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult,
};

/// The SQL below is what this function becomes, doc comment and all.
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

// The symbol DuckDB looks for when it loads the extension.
duckfn_entrypoint!("my_ext");`;

/** The SQL half of the showcase: the whole interface, with no glue in sight. */
const SQL_SAMPLE = `-- the extension loads like any other
LOAD './my_ext.duckdb_extension';

SELECT double_it(21);    -- 42
SELECT double_it(NULL);  -- NULL
SELECT double_it(13);    -- error: unlucky input`;

/**
 * The shields.io badges ask for `style=flat`, which is the rounded style; the default
 * `flat-square` draws square corners and would clash with the docs.rs badge, whose own
 * SVG is already rounded. The row has to look like one set, so the shape is decided at
 * the source rather than patched with CSS.
 */
const BADGES = [
  {
    href: 'https://crates.io/crates/duckfn',
    src: 'https://img.shields.io/crates/v/duckfn.svg?style=flat',
    alt: 'duckfn on crates.io',
  },
  {
    href: 'https://docs.rs/duckfn',
    src: 'https://docs.rs/duckfn/badge.svg',
    alt: 'API documentation on docs.rs',
  },
  {
    href: 'https://github.com/shijianjs/duckfn/blob/main/LICENSE',
    src: 'https://img.shields.io/badge/license-MIT-14459b.svg?style=flat',
    alt: 'MIT license',
  },
  {
    href: 'https://github.com/shijianjs/duckfn',
    src: 'https://img.shields.io/badge/Rust-1.86%2B-14459b.svg?style=flat',
    alt: 'Rust 1.86 or newer',
  },
];

function heroData(
  logoSrc: string,
  introHref: string,
  title: string,
  tagline: string,
): HeroData {
  return {
    logoSrc,
    title,
    tagline,
    primary: {
      label: translate({id: 'homepage.getStarted', message: 'Get started'}),
      href: introHref,
    },
    secondary: {
      label: translate({
        id: 'homepage.github',
        description: 'Home page button linking to the repository',
        message: 'GitHub',
      }),
      href: GITHUB_URL,
      icon: 'simple-icons:github',
    },
    badges: BADGES,
  };
}

function featuresData(): FeaturesData {
  return {
    sectionTitle: translate({
      id: 'homepage.features.title',
      description: 'Home page section title above the feature cards',
      message: 'Why duckfn',
    }),
    items: [
      {
        icon: 'lucide:sparkles',
        title: translate({
          id: 'homepage.features.noGlue.title',
          description: 'Home page feature card title',
          message: 'No C/C++ glue code',
        }),
        details: translate({
          id: 'homepage.features.noGlue.details',
          description: 'Home page feature card description',
          message:
            "DuckDB's C types never appear in your code. You write ordinary Rust — Option, Vec, derived structs and enums — and the attribute writes the rest.",
        }),
      },
      {
        icon: 'lucide:package',
        title: translate({
          id: 'homepage.features.noBuild.title',
          description: 'Home page feature card title',
          message: 'No local DuckDB build',
        }),
        details: translate({
          id: 'homepage.features.noBuild.details',
          description: 'Home page feature card description',
          message:
            "Depend on the headers, not on the library: the extension dispatches through DuckDB's API table at load time, so nothing is linked and cross-compilation works.",
        }),
      },
      {
        icon: 'lucide:shield-check',
        title: translate({
          id: 'homepage.features.safe.title',
          description: 'Home page feature card title',
          message: 'Safe by default',
        }),
        details: translate({
          id: 'homepage.features.safe.details',
          description: 'Home page feature card description',
          message:
            'No unsafe functions and no raw pointers inside your function bodies. The only unsafe left is in explicit manual registration.',
        }),
      },
      {
        icon: 'lucide:hash',
        title: translate({
          id: 'homepage.features.attributes.title',
          description: 'Home page feature card title',
          message: 'Attribute-driven registration',
        }),
        details: translate({
          id: 'homepage.features.attributes.details',
          description: 'Home page feature card description',
          message:
            'Annotating a function is enough. The registration items are collected for you and applied when DuckDB loads the extension.',
        }),
      },
      {
        icon: 'lucide:life-buoy',
        title: translate({
          id: 'homepage.features.panic.title',
          description: 'Home page feature card title',
          message: 'Panic-safe',
        }),
        details: translate({
          id: 'homepage.features.panic.details',
          description: 'Home page feature card description',
          message:
            'A panic inside a function body is caught and reported as a DuckDB error instead of unwinding across the FFI boundary.',
        }),
      },
      {
        icon: 'lucide:braces',
        title: translate({
          id: 'homepage.features.nested.title',
          description: 'Home page feature card title',
          message: 'Nested types included',
        }),
        details: translate({
          id: 'homepage.features.nested.details',
          description: 'Home page feature card description',
          message:
            "Vec, IndexMap, fixed-size arrays, STRUCTs and any nesting of them map to DuckDB's LIST, MAP, ARRAY and STRUCT.",
        }),
      },
    ],
  };
}

function nextStepsData(
  hrefs: readonly [string, string, string, string],
): NextStepsData {
  const [quickStart, attributes, example, architecture] = hrefs;
  return {
    sectionTitle: translate({
      id: 'homepage.next.title',
      description: 'Home page section title above the link cards',
      message: 'Where to go next',
    }),
    items: [
      {
        href: quickStart,
        title: translate({
          id: 'homepage.next.quickStart.title',
          description: 'Home page link card title',
          message: 'Quick start',
        }),
        details: translate({
          id: 'homepage.next.quickStart.details',
          description: 'Home page link card description',
          message: 'Write, build and load your first extension.',
        }),
      },
      {
        href: attributes,
        title: translate({
          id: 'homepage.next.attributes.title',
          description: 'Home page link card title',
          message: 'Attributes',
        }),
        details: translate({
          id: 'homepage.next.attributes.details',
          description: 'Home page link card description',
          message: 'Every attribute, argument and registration kind, with examples.',
        }),
      },
      {
        href: example,
        title: translate({
          id: 'homepage.next.example.title',
          description: 'Home page link card title',
          message: 'The example extension',
        }),
        details: translate({
          id: 'homepage.next.example.details',
          description: 'Home page link card description',
          message: 'One extension that uses every feature duckfn offers.',
        }),
      },
      {
        href: architecture,
        title: translate({
          id: 'homepage.next.architecture.title',
          description: 'Home page link card title',
          message: 'How it works',
        }),
        details: translate({
          id: 'homepage.next.architecture.details',
          description: 'Home page link card description',
          message: 'From DuckDB loading the library to your function answering a query.',
        }),
      },
    ],
  };
}

function CodeShowcase(): ReactNode {
  return (
    <section className={styles.sectionTint}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          <Translate
            id="homepage.showcase.title"
            description="Home page section title above the Rust and SQL code blocks">
            One Rust function = one SQL function
          </Translate>
        </Heading>
        <p className={styles.sectionLead}>
          <Translate
            id="homepage.showcase.lead"
            description="Home page paragraph introducing the Rust and SQL code blocks">
            The attribute generates the FFI wrapper, the column readers and
            writers, and the registration code. Everything on the left is safe
            Rust that you could have written for a plain library.
          </Translate>
        </p>
        <div className={styles.codeGrid}>
          <CodeBlock language="rust" title="src/lib.rs">
            {RUST_SAMPLE}
          </CodeBlock>
          <div className={styles.codeColumn}>
            <CodeBlock language="sql" title="duckdb -unsigned">
              {SQL_SAMPLE}
            </CodeBlock>
            {/* Balances the two columns, and explains the trailing comments. */}
            <p className={styles.codeCaption}>
              <Translate
                id="homepage.showcase.caption"
                description="Home page note under the SQL code block explaining the trailing comments">
                The comments are what each call returns. Loading needs -unsigned,
                because the extension talks to DuckDB's C API.
              </Translate>
            </p>
          </div>
        </div>
        <p className={styles.showcaseLinkRow}>
          <Link className={styles.showcaseLink} to="/docs/examples/side-by-side">
            <Translate
              id="homepage.showcase.link"
              description="Home page link to the side-by-side comparison page">
              Same functions, two ways: four of them written both ways
            </Translate>
            {/* The official Iconify web component (registered by
                registerDfkElements()); a string `icon` attribute is all it
                needs. createElement keeps it out of the JSX namespace. */}
            {createElement('iconify-icon', {
              icon: 'lucide:arrow-right',
              className: styles.showcaseLinkArrow,
              'aria-hidden': 'true',
            })}
          </Link>
        </p>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  // Not a hard-coded "/img/...": the site is published under /duckfn/ on GitHub
  // Pages, and only useBaseUrl adds that prefix. The dfk-* components render
  // plain anchors, so every internal href is resolved here before it is passed
  // in.
  const logoUrl = useBaseUrl('img/duckfn-logo.svg');
  const introUrl = useBaseUrl('/docs/intro');
  const nextHrefs = [
    useBaseUrl('/docs/getting-started/quick-start'),
    useBaseUrl('/docs/guide/attributes'),
    useBaseUrl('/docs/examples/duckfn'),
    useBaseUrl('/docs/internals/architecture'),
  ] as const;

  return (
    <Layout
      title={siteConfig.title}
      description="Documentation for duckfn, a Rust framework for building DuckDB extensions.">
      {/* Layout renders no <main> of its own: this is the page's only one. */}
      <main>
        {dfk(
          'dfk-hero',
          heroData(
            logoUrl,
            introUrl,
            siteConfig.title,
            translate({id: 'homepage.tagline', message: siteConfig.tagline}),
          ),
        )}
        {dfk('dfk-features', featuresData())}
        <CodeShowcase />
        {dfk('dfk-next-steps', nextStepsData(nextHrefs))}
      </main>
    </Layout>
  );
}
