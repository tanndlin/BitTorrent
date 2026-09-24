# ---------- builder ----------
FROM rust:latest AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./

# Build deps only using dummy src — this layer is cached until Cargo.toml/Cargo.lock changes
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --bin bittorrent
RUN rm -rf src

# Copy real source and touch main.rs so cargo knows to recompile
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release --bin bittorrent

# ---------- profiling ----------
# Built only when targeted (`docker compose run bittorrent-profiler`); release + debug symbols, run under samply
FROM rust:latest AS profiling-builder
WORKDIR /app

# Keep a frame pointer in every function so stacks unwind reliably even where DWARF unwind info is thin
ENV RUSTFLAGS="-C force-frame-pointers=yes"

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --profile profiling --bin bittorrent
RUN rm -rf src

COPY src ./src
RUN touch src/main.rs
RUN cargo build --profile profiling --bin bittorrent

FROM debian:trixie-slim AS profiling
WORKDIR /app

# libc6-dbg: debian's libc is stripped, so without it libc frames show up as fun_XXXXXX instead of memcpy etc.
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl xz-utils libc6-dbg && rm -rf /var/lib/apt/lists/*
RUN curl -sSL https://github.com/mstange/samply/releases/download/samply-v0.13.1/samply-x86_64-unknown-linux-gnu.tar.xz \
    | tar -xJ --strip-components=1 -C /usr/local/bin samply-x86_64-unknown-linux-gnu/samply

COPY --from=profiling-builder /app/target/profiling/bittorrent /app/bittorrent

# samply refuses to run while perf_event_paranoid > 1; the container is privileged so it can lower it.
# --presymbolicate writes symbols next to the profile so `samply load` works on the host
ENTRYPOINT ["sh", "-c", "echo 1 > /proc/sys/kernel/perf_event_paranoid && exec samply record --save-only --unstable-presymbolicate -o /profiles/profile.json.gz -- /app/bittorrent \"$@\"", "--"]

# ---------- runtime ----------
FROM debian:trixie-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bittorrent /app/bittorrent

EXPOSE 6881/tcp
EXPOSE 6881/udp

ENTRYPOINT ["/app/bittorrent"]
