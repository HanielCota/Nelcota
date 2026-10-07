# syntax=docker/dockerfile:1
# nelcota image: static (musl) binary on top of `scratch`.
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
# BuildKit caches: rebuilds only recompile what changed.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p nelcota-server \
 && cp target/release/nelcota /nelcota

FROM scratch
COPY --from=build /nelcota /usr/local/bin/nelcota
USER 65532:65532
EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/nelcota"]
CMD ["serve"]
