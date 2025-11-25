FROM registry.mida.owaas.com/docker.io/library/rust:1.89-alpine AS build
WORKDIR /build
ARG KUBESEAL_VERSION="0.32.2"
ARG TARGETPLATFORM
RUN apk add --no-cache curl libgcc musl-dev
RUN curl -OL "https://github.com/bitnami-labs/sealed-secrets/releases/download/v${KUBESEAL_VERSION:?}/kubeseal-${KUBESEAL_VERSION:?}-$(echo "${TARGETPLATFORM}" | tr '/' '-').tar.gz" && \
        tar -xvzf "kubeseal-${KUBESEAL_VERSION:?}-$(echo "${TARGETPLATFORM}" | tr '/' '-').tar.gz" kubeseal
COPY . .
RUN rustup target add "$(if [ "$(echo "${TARGETPLATFORM}" | cut -d '/' -f 2)" = "amd64" ]; then echo "x86_64"; else echo "aarch64"; fi; )-unknown-linux-musl"
RUN rustup set default-host "$(if [ "$(echo "${TARGETPLATFORM}" | cut -d '/' -f 2)" = "amd64" ]; then echo "x86_64"; else echo "aarch64"; fi; )-unknown-linux-musl"
RUN cargo build --release

FROM registry.mida.owaas.com/docker.io/library/alpine:3.22 AS run
RUN adduser -s /bin/bash -D -h /home/secret-sealer secret-sealer
COPY --from=build --chmod=0755 /build/target/release/secret-sealer-service /app/secret-sealer-service
COPY --from=build --chmod=0755 /build/kubeseal /usr/local/bin/kubeseal
WORKDIR /app
USER secret-sealer
ENTRYPOINT [ "./secret-sealer-service" ]
