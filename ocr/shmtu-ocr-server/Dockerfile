FROM rust:1.82-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    wget unzip && \
    wget -q https://github.com/microsoft/onnxruntime/releases/download/v1.20.0/onnxruntime-linux-x64-1.20.0.tgz && \
    tar xzf onnxruntime-linux-x64-1.20.0.tgz && \
    mv onnxruntime-linux-x64-1.20.0 /opt/onnxruntime && \
    rm onnxruntime-linux-x64-1.20.0.tgz && \
    apt-get purge -y wget unzip && apt-get autoremove -y && rm -rf /var/lib/apt/lists/*

ENV ORT_DYLIB_PATH=/opt/onnxruntime/lib/libonnxruntime.so
ENV LD_LIBRARY_PATH=/opt/onnxruntime/lib:${LD_LIBRARY_PATH}

COPY . .
RUN cargo build --release -p shmtu-ocr-server

FROM debian:bookworm-slim AS runtime-cpu

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /opt/onnxruntime/lib/libonnxruntime.so* /usr/lib/
COPY --from=builder /app/target/release/shmtu-ocr-server /usr/local/bin/

ENV LD_LIBRARY_PATH=/usr/lib
EXPOSE 21600 21601

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:21600/api/health || exit 1

CMD ["shmtu-ocr-server", "--model-dir", "/app/models"]
