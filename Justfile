  set dotenv-load := false

  default: lint

  fmt:
      cargo fmt --all

  fmt-check:
      cargo fmt --all --check

  clippy:
      cargo clippy --workspace --all-targets --all-features -- -D warnings

  lint:
      just fmt-check
      just clippy

  test:
      cargo test --workspace --all-features --all-targets

  check:
      cargo check --workspace --all-targets --all-features

  bench:
      cargo bench -p yps-bench

  cov:
      cargo llvm-cov --workspace --all-features --lcov --output-path target/lcov.info

  cov-html:
      cargo llvm-cov --workspace --all-features --open

  audit:
      cargo deny check bans advisories licenses sources

  ci:
      just lint
      just test
      just audit
  fuzz target="lexer" time="120":
      cargo +nightly fuzz run {{target}} -- -max_total_time={{time}}

  fuzz-all:
      just fuzz lexer
      just fuzz parser
      just fuzz fmt_roundtrip
