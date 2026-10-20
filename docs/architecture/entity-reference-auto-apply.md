# Automatic entity reference application

Parent: `d183d7c711ac2aa516a182dbcb5e12d8a58665f3`.

Explicit, uniquely resolved `@name` mentions in the current project now append the entity cover (or first linked image) to the exact REF2VA `reference_images` slot without an extra Apply click. Existing image order and video/audio inputs remain unchanged. References are deduplicated and Recipe limits remain authoritative. No image is guessed or copied into first/last-frame slots; switching generation mode remains explicit.

Storyboard rows wait for prompt save plus a 250ms debounce, then reuse the existing typed OCC save. The composer updates the existing Studio Store draft after a 250ms debounce, then uses the existing OCC autosave. IME composition postpones application until completion. Neither path submits production tasks; Generate remains explicit and readiness-gated.

Unknown/ambiguous names, missing images, invalid project identity, unsupported modes and over-limit input remain blocked. Writes and timers are scoped to project/Shot/Recipe. Save conflicts stop automatic attempts; explicit reread/retry remains available. No repeated retry loop is introduced.

Historical manifests and Runtime Package bytes remain unchanged. The predecessor guard reads its immutable parent bytes only after validating this exact successor scope, inventory, hashes and invariants. Regression probes cover forged proof, source drift, deleted files, undeclared additions and Runtime Package raw-byte mutation.

No Rust, database schema/migration, backup contract, queue authority or task model change. No GPU generation is part of this change.

Reuse: local explicitEntityMentions, entityAvatarAssetId, inputsSave and Studio Store autosave; no external dependencies or copied third-party code. A GitHub implementation search found no suitable scoped replacement.
