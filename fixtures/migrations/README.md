# Migration fixtures

Every migration in `vellum-codec-json` ships a pair of files here (guide Prompt 9, Invariant 5):

```text
fixtures/migrations/v{N}-to-v{N+1}/before.vellum   a file at format N
fixtures/migrations/v{N}-to-v{N+1}/after.vellum    the same file at format N+1
```

`cargo test -p vellum-codec-json --test migrations` runs each registered step on `before.vellum` and requires
the result to equal `after.vellum`. FORMAT_VERSION is 1, so there are no migrations and no pairs yet. The
first format change adds step 1 to `Migrations::standard()` and the first pair here.
