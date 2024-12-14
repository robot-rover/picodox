check_all:
  cd firmware && cargo check
  cd cli && cargo check
  cd bootloader && cargo check
  cd proto && cargo check

test_all:
  cd proto && cargo test
  cd cli && cargo test
