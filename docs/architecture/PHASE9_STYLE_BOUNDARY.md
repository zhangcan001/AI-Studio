# Phase9 — style ownership, not redesign

## Ordered loading

`src/app/App.css` is an eager ordered entry. Its five imports retain the original
rule and declaration sequence: shared foundation → Workflow actions → shared
model-version selector → Workflow recognition → mixed compatibility styles.
The original Tokens → Polish → Quality order in App.tsx is unchanged. Extraction
has a canonical PostCSS sequence hash, checked independently of Git history.
Do not move these imports to lazy components without cascade evidence.

Only the bounded Workflow block was assigned feature ownership. The large mixed
compatibility remainder is explicitly **DEFER**, not a new universal primitive or
a claim that all cross-feature debt disappeared. Prompt/Task provenance remains
shared in that remainder: both real callers need it. The model-version selector
has real Lab and Asset callers with identical semantics; it is shared, not owned
by Workflow. Existing multi-feature selectors retain their compatibility purpose.

## Proof-based cleanup

The retired `.studio-shell` root and its BEM elements have no production source,
template, conditional class, Advanced, legacy-locator or positive test consumer.
Phase7's negative DOM assertion is retained. Only those selectors were removed;
active startup/global/topbar/rail/polish rules remain. No other zero-candidate
selector was deleted: a source-string candidate count is **not** a liveness proof.

The 900px runtime-metrics rule was redundant with the subsequent 1200px superset
and no intervening metrics override. It alone was consolidated. All other
similar-looking UI remains separate unless semantic equivalence is demonstrated.

Bare control/heading/text presentation rules now use the zero-specificity
`:where(.studio-context-guard, .startup-screen)` host. The application's outer
host includes both V3/Advanced surfaces **and the sibling route-discard dialog**;
there are no production createPortal consumers. Global root/reset,
body, animation and token rules are intentionally retained. Adding a portal later
requires an explicit style-host decision, not an accidental global override.

Create's base label rule uses `:where(label)`, allowing the later reference class
to win without !important. Exact matching spacing/radius values in V3/Create/Runs
reuse Studio tokens with the original-value fallback. Colors and differing
fallback values are unchanged. No new token names or parallel system was added.
Dynamic inline styles remain unchanged; isolated fixed values are **DEFER** rather
than grounds for a component rewrite in this style-only phase.

## Guard and metric semantics

DEV-088 imports the style guard; there is no competing lint command. The matrix
contains the complete baseline selector inventory, classified ownership decisions,
inline candidates and final snapshots. Its explicit grandfather list is keyed by
file + at-rule context + selector, with non-expanding important/ID/deep/global
budgets. Ownership transfer is allowed only for exact existing moved selectors.
New feature files reject foreign feature roots. All unreviewed CSS changes/new
files fail the pinned snapshots; future intentional changes must update the
matrix *and* pass non-expanding debt/ownership review, not regenerate allowances.

Source-string usage counts include templates/conditions and are conservative
candidates; they are not runtime usage counts. Selector counts include each
comma-list member, including keyframes. Global means uncontained tag/universal/
pseudo rules; root/body/reset and keyframes are explicit KEEP_GLOBAL. Variable
count means custom-property declarations, including old repeated overrides, not
unique token names. `deep` means five class/pseudo specificity units or five
compounds; :where is zero and :is/:not/:has use their maximum argument.

Phase7's CSS-only freeze has an explicit Phase9 successor. Phase8 backend,
migrations, IPC signatures, every non-CSS compatibility file and **every production
TS/TSX source** remain byte-for-byte pinned. No route/component/behavior changes
are authorized by updating style snapshots.

## Acceptance budget

Ten focused local cases: eight StyleBoundary cases plus two existing
BackendBoundary cases. No full local suite. Final exact-HEAD Source-only CI is the
authoritative full suite; native acceptance uses a fresh isolated fixture and
1920/1440/1180 viewports (1024 supplementary), never real user project data.
