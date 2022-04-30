####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-gnu
RUN apt update
RUN update-ca-certificates

# Create appuser
ENV USER=pimetrics  
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"


WORKDIR /pimetrics

COPY ./ .

RUN cargo build --target x86_64-unknown-linux-gnu --release

####################################################################################################
## Final image
####################################################################################################
FROM archlinux:base-devel

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /pimetrics

# Copy our build
COPY --from=builder /pimetrics/target/x86_64-unknown-linux-gnu/release pimetrics ./


RUN pacman -Syu --noconfirm

# Use an unprivileged user.
USER pimetrics:pimetrics

CMD ["/pimetrics/pimetrics"]