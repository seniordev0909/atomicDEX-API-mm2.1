# Contributing to AtomicDEX Marketmaker

We welcome contribution from everyone in the form of suggestions, bug reports, pull requests, and feedback.
Please note we have a code of conduct, please follow it in all your interactions with the project.

## Submitting feature requests

Before uploading any changes, please make sure that the test suite passes locally before submitting a pull request with your changes.

```
cargo test --all
```

We also use [Clippy](https://github.com/rust-lang/rust-clippy) to avoid common mistakes
and we use [rustfmt](https://github.com/rust-lang/rustfmt) to make our code clear to everyone.

1. Install these tools (only once):
    ```
    rustup component add rustfmt
    rustup component add clippy
    ```
1. Format the code using rustfmt:
    ```
    cargo fmt
    ```
1. Make sure there are no warnings and errors. Run the Clippy:
    ```
    cargo clippy -- -D warnings
    ```
