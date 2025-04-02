# --- Build Stage ---
FROM rustlang/rust:nightly-slim as builder
WORKDIR /usr/src/app

# Install clang and libclang-dev to satisfy bindgen's dependency on libclang.
RUN apt-get update && apt-get install -y clang libclang-dev

# Set LIBCLANG_PATH to where libclang is installed.
# (For Debian Bookworm, clang is usually version 14; adjust if needed.)
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib

# Copy the entire workspace into the container.
COPY . .
# Change directory into the server crate.
WORKDIR /usr/src/app/server
# Build the server package in release mode.
RUN cargo build -p server --release

# --- Runtime Stage ---
FROM debian:bookworm-slim
# Copy the built binary from the builder stage.
COPY --from=builder /usr/src/app/target/release/server /usr/local/bin/server
EXPOSE 8080
CMD ["server"]
    