#!/bin/bash

# DiMA Multi-Platform Build Script
# Builds for macOS Intel, macOS Apple Silicon, and Windows x86_64

set -e

echo "🚀 Building DiMA for multiple platforms..."

# Clean previous builds
echo "🧹 Cleaning previous builds..."
cargo clean

# Build for current platform (macOS Intel)
echo "🍎 Building for macOS Intel (x86_64)..."
cargo build --release
cp target/release/dima target/release/dima-macos-intel

# Build for Apple Silicon
echo "🍎 Building for Apple Silicon (ARM64)..."
cargo build --release --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/dima target/release/dima-macos-arm64

# Build for Windows
echo "🪟 Building for Windows x86_64..."
cargo build --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/dima.exe target/release/dima-windows-x86_64.exe

echo "✅ Build complete! Executables available in target/release/:"
echo "   - dima-macos-intel (macOS Intel x86_64)"
echo "   - dima-macos-arm64 (macOS Apple Silicon)"
echo "   - dima-windows-x86_64.exe (Windows x86_64)"

# Show file sizes
echo ""
echo "📊 File sizes:"
ls -lh target/release/dima-*

echo ""
echo "🎯 All builds completed successfully!"
echo "   SIMD optimizations are enabled for all x86_64 and ARM64 targets"
echo "   Memory-mapped I/O and integer k-mer encoding included"
