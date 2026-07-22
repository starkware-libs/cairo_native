FROM debian:12

# Install dependencies
RUN apt update -y && apt install -y lsb-release \
    wget \
    curl \
    git \
    build-essential \
    libclang-dev \
    libz-dev \
    libzstd-dev \
    python3

# Install LLVM 19
RUN echo "deb http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-19 main" > /etc/apt/sources.list.d/llvm-19.list
RUN echo "deb-src http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-19 main" >> /etc/apt/sources.list.d/llvm-19.list
RUN wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add -
RUN apt update -y && apt install -y \
    libmlir-19-dev \
    libpolly-19-dev \
    llvm-19-dev \
    mlir-19-tools

# Install rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy cairo_native code
COPY . /cairo_native/

# Compile cairo_native
WORKDIR /cairo_native/
ENV MLIR_SYS_190_PREFIX=/usr/lib/llvm-19
ENV LLVM_SYS_191_PREFIX=/usr/lib/llvm-19
ENV TABLEGEN_190_PREFIX=/usr/lib/llvm-19
# The cargo registry/git and target dirs are mounted as BuildKit caches so the
# (slow, release-LTO) Rust build is incremental across CI runs. They are cache
# mounts rather than image layers: the contents are not part of the final image,
# which only needs to prove the build succeeds.
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/cairo_native/target \
    make deps
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/cairo_native/target \
    make build
