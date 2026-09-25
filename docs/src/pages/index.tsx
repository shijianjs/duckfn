import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import Translate from '@docusaurus/Translate';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import CodeBlock from '@theme/CodeBlock';
import Heading from '@theme/Heading';
import Layout from '@theme/Layout';

import {
  ArrowRightIcon,
  BracesIcon,
  GitHubIcon,
  HashIcon,
  LifeBuoyIcon,
  PackageIcon,
  ShieldCheckIcon,
  SparklesIcon,
} from '../components/icons';
import styles from './index.module.css';

/**
 * The landing page: hero, features, a Rust/SQL showcase and the "where next"
 * cards. Laid out like the VitePress home (centred hero, feature grid, code
 * sample) but built from the theme's own primitives — `Layout`, `Heading`,
 * `CodeBlock` — and from Infima variables, so light and dark mode come for free
 * and nothing overrides the docs pages.
 *
 * Copy lives in `<Translate>` so both locales stay in sync; the Chinese strings
 * are in `i18n/zh-Hans/code.json` under the same `homepage.*` keys.
 */

/** Icons only need a class name: colour comes from `currentColor`. */
type IconComponent = (props: {className?: string}) => ReactNode;

const GITHUB_URL = 'https://github.com/shijianjs/duckfn';

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

const FEATURES: {
  key: string;
  Icon: IconComponent;
  title: ReactNode;
  details: ReactNode;
}[] = [
  {
    key: 'noGlue',
    Icon: SparklesIcon,
    title: (
      <Translate
        id="homepage.features.noGlue.title"
        description="Home page feature card title">
        No C/C++ glue code
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.features.noGlue.details"
        description="Home page feature card description">
        DuckDB's C types never appear in your code. You write ordinary Rust —
        Option, Vec, derived structs and enums — and the attribute writes the
        rest.
      </Translate>
    ),
  },
  {
    key: 'noBuild',
    Icon: PackageIcon,
    title: (
      <Translate
        id="homepage.features.noBuild.title"
        description="Home page feature card title">
        No local DuckDB build
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.features.noBuild.details"
        description="Home page feature card description">
        Depend on the headers, not on the library: the extension dispatches
        through DuckDB's API table at load time, so nothing is linked and
        cross-compilation works.
      </Translate>
    ),
  },
  {
    key: 'safe',
    Icon: ShieldCheckIcon,
    title: (
      <Translate
        id="homepage.features.safe.title"
        description="Home page feature card title">
        Safe by default
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.features.safe.details"
        description="Home page feature card description">
        No unsafe functions and no raw pointers inside your function bodies. The
        only unsafe left is in explicit manual registration.
      </Translate>
    ),
  },
  {
    key: 'attributes',
    Icon: HashIcon,
    title: (
      <Translate
        id="homepage.features.attributes.title"
        description="Home page feature card title">
        Attribute-driven registration
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.features.attributes.details"
        description="Home page feature card description">
        Annotating a function is enough. The registration items are collected
        for you and applied when DuckDB loads the extension.
      </Translate>
    ),
  },
  {
    key: 'panic',
    Icon: LifeBuoyIcon,
    title: (
      <Translate
        id="homepage.features.panic.title"
        description="Home page feature card title">
        Panic-safe
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.features.panic.details"
        description="Home page feature card description">
        A panic inside a function body is caught and reported as a DuckDB error
        instead of unwinding across the FFI boundary.
      </Translate>
    ),
  },
  {
    key: 'nested',
    Icon: BracesIcon,
    title: (
      <Translate
        id="homepage.features.nested.title"
        description="Home page feature card title">
        Nested types included
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.features.nested.details"
        description="Home page feature card description">
        Vec, IndexMap, fixed-size arrays, STRUCTs and any nesting of them map to
        DuckDB's LIST, MAP, ARRAY and STRUCT.
      </Translate>
    ),
  },
];

const NEXT_STEPS: {to: string; title: ReactNode; details: ReactNode}[] = [
  {
    to: '/docs/getting-started/quick-start',
    title: (
      <Translate
        id="homepage.next.quickStart.title"
        description="Home page link card title">
        Quick start
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.next.quickStart.details"
        description="Home page link card description">
        Write, build and load your first extension.
      </Translate>
    ),
  },
  {
    to: '/docs/guide/attributes',
    title: (
      <Translate
        id="homepage.next.attributes.title"
        description="Home page link card title">
        Attributes
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.next.attributes.details"
        description="Home page link card description">
        Every attribute, argument and registration kind, with examples.
      </Translate>
    ),
  },
  {
    to: '/docs/examples/duckfn-quack',
    title: (
      <Translate
        id="homepage.next.example.title"
        description="Home page link card title">
        The example extension
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.next.example.details"
        description="Home page link card description">
        One extension that uses every feature duckfn offers.
      </Translate>
    ),
  },
  {
    to: '/docs/internals/architecture',
    title: (
      <Translate
        id="homepage.next.architecture.title"
        description="Home page link card title">
        How it works
      </Translate>
    ),
    details: (
      <Translate
        id="homepage.next.architecture.details"
        description="Home page link card description">
        From DuckDB loading the library to your function answering a query.
      </Translate>
    ),
  },
];

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

function Hero(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  // Not a hard-coded "/img/...": the site is published under /duckfn/ on GitHub
  // Pages, and only useBaseUrl adds that prefix.
  const logoUrl = useBaseUrl('img/duckfn-logo.svg');

  return (
    <section className={styles.hero}>
      <div className={styles.heroInner}>
        {/* The <h1> below spells out the name, so the mark is decorative. */}
        <span className={styles.logoStage}>
          <img
            className={styles.logo}
            src={logoUrl}
            alt=""
            width={480}
            height={480}
          />
        </span>
        <Heading as="h1" className={styles.title}>
          {siteConfig.title}
        </Heading>
        <p className={styles.tagline}>
          <Translate id="homepage.tagline">{siteConfig.tagline}</Translate>
        </p>
        <div className={styles.actions}>
          <Link className={styles.buttonPrimary} to="/docs/intro">
            <Translate id="homepage.getStarted">Get started</Translate>
          </Link>
          <Link
            className={styles.buttonSecondary}
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer">
            <GitHubIcon className={styles.buttonIcon} />
            <Translate id="homepage.github" description="Home page button linking to the repository">
              GitHub
            </Translate>
          </Link>
        </div>
        <div className={styles.badges}>
          {BADGES.map((badge) => (
            <Link
              className={styles.badge}
              href={badge.href}
              key={badge.href}
              target="_blank"
              rel="noopener noreferrer">
              <img className={styles.badgeImage} src={badge.src} alt={badge.alt} />
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

function Features(): ReactNode {
  return (
    <section className={styles.section}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          <Translate
            id="homepage.features.title"
            description="Home page section title above the feature cards">
            Why duckfn
          </Translate>
        </Heading>
        <div className={styles.featureGrid}>
          {FEATURES.map(({key, Icon, title, details}) => (
            <article className={styles.featureCard} key={key}>
              <span className={styles.featureIconChip}>
                <Icon className={styles.featureIcon} />
              </span>
              <Heading as="h3" className={styles.featureTitle}>
                {title}
              </Heading>
              <p className={styles.featureDetails}>{details}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
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
            <ArrowRightIcon className={styles.showcaseLinkArrow} />
          </Link>
        </p>
      </div>
    </section>
  );
}

function NextSteps(): ReactNode {
  return (
    <section className={styles.section}>
      <div className={styles.sectionInner}>
        <Heading as="h2" className={styles.sectionTitle}>
          <Translate
            id="homepage.next.title"
            description="Home page section title above the link cards">
            Where to go next
          </Translate>
        </Heading>
        <div className={styles.nextGrid}>
          {NEXT_STEPS.map(({to, title, details}) => (
            <Link className={styles.nextCard} key={to} to={to}>
              <span className={styles.nextCardBody}>
                <span className={styles.nextCardTitle}>{title}</span>
                <span className={styles.nextCardDetails}>{details}</span>
              </span>
              <ArrowRightIcon className={styles.nextCardArrow} />
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();

  return (
    <Layout
      title={siteConfig.title}
      description="Documentation for duckfn, a Rust framework for building DuckDB extensions.">
      {/* Layout renders no <main> of its own: this is the page's only one. */}
      <main>
        <Hero />
        <Features />
        <CodeShowcase />
        <NextSteps />
      </main>
    </Layout>
  );
}
