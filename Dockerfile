# Use a base image compatible with your Linux executable
FROM ubuntu:latest AS builder
ARG TARGETARCH

FROM builder AS builder_amd64
ENV ARCH=x86_64
FROM builder AS builder_arm64
ENV ARCH=aarch64
FROM builder AS builder_riscv64
ENV ARCH=riscv64

FROM builder_${TARGETARCH} AS build

# Copy the ord executable into the container
EXPOSE 8080
COPY ./ord /usr/local/bin/ord

# Set executable permissions for ord
RUN chmod +x /usr/local/bin/ord

# Set the command to run when the container starts
CMD ["/bin/sh", "-c", "\
    if [ \"$TESTNET\" = \"true\" ]; then \
        /usr/local/bin/ord --server --http-port 8080 --testnet $EXTRA_PARAMS \
    ; else \
        /usr/local/bin/ord --server --http-port 8080 $EXTRA_PARAMS \
    ; fi"]
