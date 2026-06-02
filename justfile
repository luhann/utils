target_dir := join(home_dir(), "bin")

utils := replace(
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

# Cargo fmt
fmt:
  @cargo fmt

# Clean cargo artefacts
clean:
  @cargo clean

# Build everything
build:
    cargo build --release

# Install everything automatically
install-all: build
    @mkdir -p {{target_dir}}
    @for util in {{utils}}; do \
        install -vDm755 "target/x86_64-unknown-linux-musl/release/$util" "{{target_dir}}/$util"; \
    done

# Install just a single specific utility
install utility:
    @echo "Building and installing {{utility}}..."
    @cargo build --release -p {{utility}}
    @mkdir -p {{target_dir}}
    @install -vDm755 "target/x86_64-unknown-linux-musl/release/{{utility}}" "{{target_dir}}/{{utility}}"; \

# Run all tests in release mode
test:
  cargo test --release
