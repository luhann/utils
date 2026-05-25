target_dir := join(home_dir(), "bin")

bins := replace(
    replace_regex(
        replace_regex(
            shell("fd '^main\\.rs$' crates/ -X dirname"),
            "(?m)^crates/", ""
        ),
        "(?m)/src$", ""
    ),
    "\n", " "
)

default:
    @just --list

# Clean cargo artefacts
clean:
  cargo clean

# Build everything
build-all:
    cargo build --release

# Install everything automatically
install-all: build-all
    @mkdir -p {{target_dir}}
    @for bin in {{bins}}; do \
        cp -v "target/release/$bin" {{target_dir}}/; \
    done

# Install just a single specific utility
install utility:
    @echo "Building and installing {{utility}}..."
    @cargo build --release -p {{utility}}
    @mkdir -p {{target_dir}}
    @cp -v "target/release/{{utility}}" {{target_dir}}/

# Run all tests in release mode
test:
  cargo test --release
