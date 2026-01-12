#!/bin/bash
# Run Vixy integration tests with Docker Compose
#
# This script:
#   1. Starts Docker Compose infrastructure
#   2. Builds and starts Vixy
#   3. Runs integration tests
#   4. Cleans up
#
# Usage:
#   ./scripts/run-integration-tests.sh [--keep-running]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DOCKER_DIR="$PROJECT_DIR/docker"

KEEP_RUNNING=false
if [[ "$1" == "--keep-running" ]]; then
    KEEP_RUNNING=true
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

echo_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

echo_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Cleanup function
cleanup() {
    if [[ "$KEEP_RUNNING" == "false" ]]; then
        echo_info "Cleaning up..."

        # Stop Vixy if running
        if [[ -n "$VIXY_PID" ]]; then
            echo_info "Stopping Vixy (PID: $VIXY_PID)..."
            kill "$VIXY_PID" 2>/dev/null || true
            wait "$VIXY_PID" 2>/dev/null || true
        fi

        # Stop Docker Compose
        echo_info "Stopping Docker Compose..."
        cd "$DOCKER_DIR" && docker-compose down 2>/dev/null || true

        # Restart any stopped containers from failover tests
        docker start vixy-geth-primary 2>/dev/null || true
        docker start vixy-cl-mock-primary 2>/dev/null || true
    else
        echo_info "Keeping infrastructure running (--keep-running specified)"
        echo_info "To stop: cd docker && docker-compose down"
        if [[ -n "$VIXY_PID" ]]; then
            echo_info "Vixy PID: $VIXY_PID"
        fi
    fi
}

trap cleanup EXIT

# Check prerequisites
echo_info "Checking prerequisites..."

if ! command -v docker &> /dev/null; then
    echo_error "Docker is not installed. Please install Docker first."
    exit 1
fi

if ! docker info &> /dev/null; then
    echo_error "Docker daemon is not running. Please start Docker."
    exit 1
fi

if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo_error "Docker Compose is not installed. Please install Docker Compose."
    exit 1
fi

# Start Docker Compose
echo_info "Starting Docker Compose infrastructure..."
cd "$DOCKER_DIR"

# Use 'docker compose' (V2) or 'docker-compose' (V1)
if docker compose version &> /dev/null; then
    COMPOSE_CMD="docker compose"
else
    COMPOSE_CMD="docker-compose"
fi

$COMPOSE_CMD up -d

echo_info "Waiting for containers to be healthy..."
sleep 5

# Check container health
for container in vixy-geth-primary vixy-geth-secondary; do
    if docker ps --filter "name=$container" --filter "status=running" | grep -q "$container"; then
        echo_info "  ✓ $container is running"
    else
        echo_warn "  ✗ $container is not running"
    fi
done

# Build Vixy
echo_info "Building Vixy..."
cd "$PROJECT_DIR"
cargo build --release

# Start Vixy in background
echo_info "Starting Vixy..."
RUST_LOG=info ./target/release/vixy --config "$DOCKER_DIR/vixy-integration.toml" &
VIXY_PID=$!

# Wait for Vixy to start
echo_info "Waiting for Vixy to start..."
for i in {1..30}; do
    if curl -s http://127.0.0.1:8080/status > /dev/null 2>&1; then
        echo_info "Vixy is ready!"
        break
    fi
    if [[ $i -eq 30 ]]; then
        echo_error "Vixy failed to start within 30 seconds"
        exit 1
    fi
    sleep 1
done

# Run integration tests
echo_info "Running integration tests..."
cd "$PROJECT_DIR"
VIXY_SKIP_INTEGRATION_CHECK=1 cargo test --test integration_cucumber -- --color always
TEST_RESULT=$?

if [[ $TEST_RESULT -eq 0 ]]; then
    echo_info "Integration tests passed!"
else
    echo_error "Integration tests failed!"
fi

exit $TEST_RESULT
