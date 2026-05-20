# Stage 1: Builder
FROM rust:1.88-bookworm AS builder
WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y pkg-config libpq-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests and source
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

# Build - Cargo.lock already has compatible versions
RUN cargo build --release && rm -rf src

# Stage 2: Runtime
FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/yomu-gamification-engine /app/yomu-gamification-engine
EXPOSE 8081
USER nonroot:nonroot
ENTRYPOINT ["/app/yomu-gamification-engine"]