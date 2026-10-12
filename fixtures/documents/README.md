# Document fixtures

Canonical `.vellum` files, all marked `TEST DATA` in `meta.name`. Loading one and saving it must give the same
bytes (`cargo test -p vellum-codec-json --test fixtures`). Between them they use every kind of node, paint,
effect, variable and style the format has.

To change one, edit it and rewrite it in canonical form:

```sh
cargo run -p vellum-codec-json --example canonicalize -- fixtures/documents/*.vellum
cargo run -p vellum-codec-json --example canonicalize -- --check fixtures/documents/*.vellum
```
