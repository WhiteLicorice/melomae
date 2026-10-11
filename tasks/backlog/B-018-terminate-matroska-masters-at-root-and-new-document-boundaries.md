# B-018 — Terminate Matroska masters at root and new-document boundaries

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

RFC 8794 §6.2 ends an unknown-size master at the end of a finite parent, at a sibling or parent element, at a root element, or at a new EBML header. The task 03 scanner (`decode/matroska.rs`, `parse_segment`) handles finite parents and sibling Clusters. It has no explicit root or new-document branch, and it skips such elements as children. A test with two documents decodes, but no test asserts where the first Segment ends.

## Why this does not block MVP

No review reproduced a wrong result from a realistic file. Chained WebM documents are rare outside live streams.

## Expected behavior

An unknown-size Segment or Cluster ends at a root element or a new EBML header, per RFC 8794 §6.2.

## Promotion condition

Promote this task with live-stream recording support, or with the next change to the scanner.

## Acceptance criteria

- [ ] Tests assert the return boundary of `parse_segment` for a root element and a new EBML header.
- [ ] A chained two-document WebM decodes the first document with the correct trim.
