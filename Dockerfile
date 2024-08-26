FROM rust:1-bullseye AS builder
WORKDIR /usr/src/paidy
COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/paidy-assignment /usr/local/bin/paidy-assignment
CMD ["paidy-assignment"]

