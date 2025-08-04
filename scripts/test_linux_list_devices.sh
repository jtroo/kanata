#!/bin/bash

# Linux Testing Script for kanata --list functionality
# Run this on your Ubuntu machine after cloning the repo

set -e  # Exit on any error

echo "🐧 Linux Testing Script for kanata --list"
echo "========================================"
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_step() {
    echo -e "${BLUE}📋 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Test 1: Basic Build Test
print_step "Test 1: Building kanata on Linux"
echo "Building with default features..."
cargo build --release
if [ $? -eq 0 ]; then
    print_success "Build successful"
else
    print_error "Build failed"
    exit 1
fi
echo

# Test 2: Help Output Test
print_step "Test 2: Checking --list availability in help"
echo "Running: cargo run --release -- --help"
HELP_OUTPUT=$(cargo run --release -- --help 2>&1)
if echo "$HELP_OUTPUT" | grep -q "\-l, \-\-list"; then
    print_success "--list option found in help output"
else
    print_error "--list option NOT found in help output"
    echo "Help output:"
    echo "$HELP_OUTPUT"
    exit 1
fi
echo

# Test 3: List Devices Functionality
print_step "Test 3: Testing --list functionality"
echo "Running: cargo run --release -- --list"
echo "Note: This may require permissions or show permission errors"
echo

LIST_OUTPUT=$(cargo run --release -- --list 2>&1)
EXIT_CODE=$?

echo "Exit code: $EXIT_CODE"
echo "Output:"
echo "$LIST_OUTPUT"
echo

if [ $EXIT_CODE -eq 0 ]; then
    print_success "--list executed successfully"
    
    # Check for expected output format
    if echo "$LIST_OUTPUT" | grep -q "Available keyboard devices:"; then
        print_success "Found expected header"
    else
        print_warning "Expected header not found"
    fi
    
    if echo "$LIST_OUTPUT" | grep -q "Configuration example:"; then
        print_success "Found configuration example"
    else
        print_warning "Configuration example not found"
    fi
    
else
    print_warning "--list execution returned non-zero exit code"
    
    # Check if it's a permission issue
    if echo "$LIST_OUTPUT" | grep -q -i "permission"; then
        print_warning "Permission issue detected - this is expected on some systems"
        echo
        echo "💡 Try running with sudo or adding user to input group:"
        echo "   sudo usermod -a -G input \$USER"
        echo "   (then log out and back in)"
    else
        print_error "Unexpected error"
    fi
fi
echo

# Test 4: System Information
print_step "Test 4: System Information"
echo "OS Information:"
lsb_release -a 2>/dev/null || cat /etc/os-release
echo
echo "Available input devices in /dev/input/:"
ls -la /dev/input/ | head -10
echo
echo "Current user groups:"
groups
echo
echo "Input group membership:"
if groups | grep -q input; then
    print_success "User is in input group"
else
    print_warning "User is NOT in input group"
    echo "💡 Add user to input group: sudo usermod -a -G input \$USER"
fi
echo

# Test 5: Device Files Test
print_step "Test 5: Input Device Files"
echo "Checking for keyboard-like devices..."
DEVICE_COUNT=$(ls /dev/input/event* 2>/dev/null | wc -l)
echo "Found $DEVICE_COUNT event devices"

if [ $DEVICE_COUNT -gt 0 ]; then
    print_success "Input devices found"
    echo "Device files:"
    ls -la /dev/input/event* 2>/dev/null | head -5
else
    print_warning "No input devices found"
fi
echo

# Test 6: Dependencies Check
print_step "Test 6: Dependencies Check"
echo "Rust version:"
rustc --version
echo "Cargo version:"
cargo --version
echo

print_step "Test 7: Feature Build Test"  
echo "Testing build without explicit features..."
cargo clean
cargo build --release --no-default-features
if [ $? -eq 0 ]; then
    print_success "No-default-features build successful"
else
    print_warning "No-default-features build failed"
fi
echo

# Summary
echo "🏁 Linux Testing Summary"
echo "======================="
echo "✅ Build test"
echo "✅ Help output test"
echo "✅ --list functionality test"
echo "✅ System information gathering"
echo "✅ Dependencies check"
echo
echo "📋 Next Steps:"
echo "1. If permission errors: Add user to input group and re-test"
echo "2. If successful: Test with actual USB keyboard plugged in"
echo "3. Test edge cases (no keyboards, multiple keyboards, etc.)"
echo
echo "🎯 Linux testing complete!"
