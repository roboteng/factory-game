A factory/automation game inspired by Factorio.

## Code Constraints

These constraints aren't well expresed in the type system, but are still important.

### All calls to `.trigger()` must be done in the `Preupdate` schedule

We rely on `Commands` to get flushed moving from `Preupdate` to `Update`.
This happens automatically when the `Preupdate` schedule is finished.
