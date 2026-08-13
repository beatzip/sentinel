#!/usr/bin/env bash
# ============================================================
#  Sentinel AI - simple launcher for Linux / macOS
#  Usage:
#    ./run.sh analyze match.dem        (анализ, с учётом памяти)
#    ./run.sh learn   match.dem        (анализ + обучение памяти)
#    ./run.sh memory                  (показать, чему научилось)
#    ./run.sh memory reset            (очистить память)
#    ./run.sh build                   (только собрать)
# ============================================================
set -e

# Build once if the binary is missing.
if [ ! -f "target/release/sentinel" ]; then
  echo "Building Sentinel AI (first run, release)..."
  cargo build --release
fi

if [ $# -eq 0 ]; then
  echo "Usage: ./run.sh <analyze|learn|memory|build> [args]"
  echo ""
  echo "  ./run.sh analyze match.dem     Analyze a demo (uses memory if present)"
  echo "  ./run.sh learn   match.dem     Analyze and train memory"
  echo "  ./run.sh memory               Show what Sentinel learned"
  echo "  ./run.sh memory reset         Clear memory"
  echo "  ./run.sh build                Rebuild the binary"
  exit 0
fi

if [ "$1" = "build" ]; then
  cargo build --release
  exit $?
fi

./target/release/sentinel "$@"
