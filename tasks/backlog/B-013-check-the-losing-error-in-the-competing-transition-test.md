# B-013 — Check the losing error in the competing-transition test

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 14

## Impact

`only_one_of_two_competing_transitions_wins` in `crates/melomae-core/src/store/race_tests.rs` counts the successful results. It does not check the error of the losing call. A database lock error satisfies the count, so a regression that turns the typed rejection into a lock error passes this test. The third task 14 review found this (P3).

## Why this does not block MVP

The production conflict path returns `StoreError::IllegalTransition`. The 98-case matrix and the cancellation race test check typed rejections. The test also checks the final state after a reopen.

## Expected behavior

The test asserts that the losing call returns `Err(StoreError::IllegalTransition { from: Restoring, to: Restoring })`. It keeps the success count and the final-state check.

## Promotion condition

Promote this task when the next change touches `transition_job` or `race_tests.rs`.

## Acceptance criteria

- [ ] The test fails when the loser returns any error other than `IllegalTransition`.
- [ ] `cargo test -p melomae-core store` passes.
