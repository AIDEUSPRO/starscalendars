#!/bin/bash
# Frontend Development: Build WASM + Start Vite
# Только для разработки фронтенда сцены

set -e

echo "🎨 Frontend Development Setup"
echo "============================"

# Check if dependencies are installed
if [[ ! -d "node_modules" ]] || [[ ! -d "frontend/node_modules" ]]; then
    echo "📥 Installing dependencies..."
    pnpm install
    echo "✅ Dependencies installed"
    echo ""
fi

# Build WASM first
echo "📦 Building WASM astronomical module..."
./scripts/build-wasm.sh

echo ""
echo "🚀 Starting Vite development server..."
echo "   URL: http://localhost:3000"
echo "   Press Ctrl+C to stop"
echo ""

# Start frontend dev server
cd frontend && pnpm run dev