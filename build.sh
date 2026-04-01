#!/usr/bin/env bash
set -e

echo "====================================="
echo " Building rtop (Rust + Vue 3)"
echo "====================================="

echo "[1/2] Building Web UI..."
cd web
npm install
npm run build
cd ..

echo "[2/2] Building Rust Backend..."
cargo build --release --workspace

echo "====================================="
echo " Build complete! "
echo " Executable is at: ./target/release/rtop"
echo "====================================="
