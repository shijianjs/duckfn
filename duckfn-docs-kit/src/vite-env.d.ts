// Ambient module type for the Vite `?inline` CSS import in `styles.ts`. Vite
// injects this file into the build automatically; `tsc` picks it up through the
// `include: ["src"]` glob, which is what keeps `npm run typecheck` and the
// emitted `.d.ts` files working without a `vite/client` lib reference.
declare module '*.css?inline' {
  const css: string;
  export default css;
}
