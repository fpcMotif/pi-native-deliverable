#!/bin/bash
RUST_BACKTRACE=1 cargo test --test tool_sandbox -- --nocapture
