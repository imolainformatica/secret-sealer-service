FROM registry.mida.owaas.com/docker.io/library/rust:1.89-alpine AS build
WORKDIR /build
ARG TARGETPLATFORM
RUN apk add --no-cache libgcc musl-dev
COPY . .
RUN rustup target add "$(if [ "$(echo "${TARGETPLATFORM}" | cut -d '/' -f 2)" = "amd64" ]; then echo "x86_64"; else echo "aarch64"; fi; )-unknown-linux-musl"
RUN rustup set default-host "$(if [ "$(echo "${TARGETPLATFORM}" | cut -d '/' -f 2)" = "amd64" ]; then echo "x86_64"; else echo "aarch64"; fi; )-unknown-linux-musl"
RUN cargo build --release

FROM registry.mida.owaas.com/docker.io/library/alpine:3.22 AS kubeseal
WORKDIR /build
ARG KUBESEAL_VERSION="0.33.1"
ARG TARGETPLATFORM
RUN apk --no-cache add curl
RUN curl -OL "https://github.com/bitnami-labs/sealed-secrets/releases/download/v${KUBESEAL_VERSION:?}/kubeseal-${KUBESEAL_VERSION:?}-$(echo "${TARGETPLATFORM}" | tr '/' '-').tar.gz" && \
        tar -xvzf "kubeseal-${KUBESEAL_VERSION:?}-$(echo "${TARGETPLATFORM}" | tr '/' '-').tar.gz" kubeseal

FROM registry.mida.owaas.com/docker.io/library/alpine:3.22 AS libs

FROM scratch AS minimal
COPY --from=libs --chmod=0555 /bin/busybox /bin/busybox
COPY --from=libs --chmod=755 /lib/ld-musl-x86_64.so.1 /lib/ld-musl-x86_64.so.1
COPY --from=libs /lib/libc.musl-x86_64.so.1 /lib/libc.musl-x86_64.so.1
RUN [ "/bin/busybox", "mkdir", "-p", "/etc" ]
RUN [ "/bin/busybox", "touch", "/etc/passwd" ]
COPY --from=build --chmod=0755 --chown=1001:1001 /build/target/release/secret-sealer-service /bin/secret-sealer-service
COPY --from=kubeseal --chmod=0755 /build/kubeseal /bin/kubeseal
RUN [ "/bin/busybox", "adduser", "-D", "-u", "1001", "secret-sealer" ]
RUN [ "/bin/busybox", "rm", "/bin/busybox" ]
USER secret-sealer
ENTRYPOINT [ "/bin/secret-sealer-service" ]
