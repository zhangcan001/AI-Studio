# Await generated-video confirmation metadata in CI

Parent: `719de4986385fd9b9c25944267f16453689f9921`.

CI run 39446083593 failed only Frontend tests (2/3), in CreateLibraryBind.phase6b.test.tsx, because the generated-video-confirmation test awaited a disabled compatible-slot button before synchronously querying a checkbox that only exists after asynchronous source metadata resolution.

The production controller intentionally renders compatible slots disabled while metadata is unknown. The test now controls that real asynchronous seam with a deferred metadata fixture: before resolution, confirm is absent, Apply is disabled and inputsSave has not run. After resolution, it awaits the actual unchecked confirmation checkbox, verifies Apply still disabled/no save, explicitly checks consent, then verifies the unchanged exact reference_videos-only OCC payload and no generation.

The same controlled schedule fails the prior synchronous getByRole query, and passes with findByRole. No timeout, assertion, product behavior or safety policy is weakened. No UI/Rust implementation, schema, migration, Backup format, Runtime Package or workflow YAML changes.

Historical proof manifests remain immutable. The new exact-parent successor validates approved hashes/inventory and projects parent bytes into the predecessor reader. Forged proof, source mutation, deletion, illegal additions and Runtime Package raw-byte mutation remain negative probes.
