FROM registry.mida.owaas.com/docker.io/library/rust:1.89 AS build
WORKDIR /build
ARG KUBESEAL_VERSION="0.32.2"
RUN curl -OL "https://github.com/bitnami-labs/sealed-secrets/releases/download/v${KUBESEAL_VERSION:?}/kubeseal-${KUBESEAL_VERSION:?}-linux-amd64.tar.gz" && \
        tar -xvzf kubeseal-${KUBESEAL_VERSION:?}-linux-amd64.tar.gz kubeseal
COPY . .
RUN cargo build --release

FROM registry.mida.owaas.com/docker.io/library/debian:stable AS run
RUN useradd -ms /bin/bash secret-sealer
COPY --from=build --chmod=0755 /build/target/release/secret-sealer-service /app/secret-sealer-service
COPY --from=build --chmod=0755 /build/kubeseal /usr/local/bin/kubeseal
WORKDIR /app
USER secret-sealer
ENTRYPOINT [ "./secret-sealer-service" ]

