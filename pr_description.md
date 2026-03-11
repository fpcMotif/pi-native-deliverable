🎯 **What:** Replaced usages of `.unwrap()` and `.unwrap_err()` in `crates/pi-protocol/src/rpc.rs` tests with `.expect("Failed to parse JSON")` and `.expect_err("Expected error but got Ok")` respectively.

💡 **Why:** This improves the maintainability and debuggability of the test suite. If a test fails due to an unexpected unwrap, it now panics with a clear, descriptive message rather than a generic unwrap panic, making it easier to diagnose the root cause of test failures.

✅ **Verification:** Verified by running the `cargo test -p pi-protocol --all-features` test suite locally, ensuring all tests pass without issues, and formatted the code with `cargo fmt --all`.

✨ **Result:** The test code in `rpc.rs` is now more robust and aligns better with idiomatic Rust code health standards for panicking on failure.
