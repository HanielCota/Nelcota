# syntax=docker/dockerfile:1
# Release image: the static (musl) binary the release workflow already built
# for each architecture, on top of `scratch`. No compilation here, so the
# arm64 image does not rebuild everything under QEMU, and the image runs the
# exact binary published on the release page. The local build is ../Dockerfile.
FROM scratch
ARG TARGETARCH
COPY --chmod=755 dist/nelcota-${TARGETARCH} /usr/local/bin/nelcota
USER 65532:65532
EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/nelcota"]
CMD ["serve"]
