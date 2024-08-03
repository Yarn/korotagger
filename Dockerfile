FROM alpine:3.20 AS builder
WORKDIR /build
COPY . .
# in korotagger repo dir
RUN apk add cargo pkgconfig openssl-dev
RUN cargo build --release

# Run the image from alpine now that it's built
FROM alpine:3.20 AS runtime

RUN apk add --no-cache libgcc postgresql-client
WORKDIR /app
COPY --from=builder /build/target/release/korotagger /app/korotagger
COPY entrypoint.sh /app/entrypoint.sh
COPY migrations /app/migrations
RUN chmod +x /app/entrypoint.sh

# binary at target/release/korotagger
ENTRYPOINT ["/app/entrypoint.sh"]
