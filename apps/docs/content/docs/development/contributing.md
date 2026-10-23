---
title: "Contributing"
description: "How a change to {{brand}} is made, tested, recorded and reviewed: the working method, the reports, the reference-code notes and the cleanup after each task."
---

The project is built in small, verified steps. A change is made in the order below, and a change that skips a step is sent back.

## 1. Read the rules

Read the repository's `AGENTS.md`, then the build guide's invariants and the open items in `reports.md`. The open items say what is already known
to be unfinished, so a change does not claim to finish something that is still open.

## 2. Write the test first

Write the failing test before the code, and run it to see it fail. A test that passes before the change proves nothing. Make the smallest coherent
change that makes the test pass. Never weaken a test to make it pass, and never delete a failing test to get a green run.

## 3. Verify the change

Run the gates that cover the change, as the [verification](/docs/development/verification) page lists. Run the linters and the type checks for the
packages you touched. Keep the output of a failure, and read the first failure first.

## 4. Record the change

A change that alters how the product behaves updates `reports.md` before it is proposed. The entry says what was built, what differs from the guide,
what failed, what could not be verified and why, and the exact commands that passed. Keep it short and factual.

A change to the architecture adds a decision record, and the version baseline when a version changes. The architecture pages are changed only through
those records.

## 5. Take reference code with care

Read a reference repository before you implement a hard problem that it solves. Put a short note in the pull request: what the reference does, what the
project keeps, and what it changes and why. Port code only from a project whose licence allows it, with a header that names the source repository, the
path and the commit, and an entry in the third-party notices. The [clean room and licensing](/docs/development/clean-room-and-licensing) page lists the rules.

## 6. Clean up the build output

After a task's verification, run `pnpm clean` once nothing is building or testing. It removes build output and keeps every source file, the dependencies, the
references and the design cache. Report what was removed, if you are reporting on the task.

## 7. Propose the change

Propose the change with a description of what it does, which gates ran, which lanes were skipped and why, and what a reviewer should look at first. Keep the
change to one purpose. A change that mixes a refactor, a feature and a fix is harder to verify and harder to revert.

## Things a reviewer checks

- The test was written first, and it failed before the change.
- No specification file was edited.
- The schema snapshot diff, if any, is additive.
- The copy is the exact copy from the guide, and it has no exclamation marks or claims of taste.
- The new code has no colour literals in the interface, and no vendor outside its adapter.
- The reports entry says what was not verified.
