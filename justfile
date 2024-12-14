check-all:
  cd firmware && cargo check
  cd cli && cargo check
  cd bootloader && cargo check
  cd proto && cargo check

test-all:
  cd proto && cargo test
  cd cli && cargo test

fix-all:
  cd firmware && cargo fix --allow-dirty
  cd cli && cargo fix --allow-dirty
  cd bootloader && cargo fix --allow-dirty
  cd proto && cargo fix --allow-dirty

fmt-check-all:
  cd firmware && cargo fmt -- --check
  cd cli && cargo fmt -- --check
  cd bootloader && cargo fmt -- --check
  cd proto && cargo fmt -- --check

fmt-all:
  cd firmware && cargo fmt
  cd cli && cargo fmt
  cd bootloader && cargo fmt
  cd proto && cargo fmt

verify-commit: check-all fmt-check-all test-all
