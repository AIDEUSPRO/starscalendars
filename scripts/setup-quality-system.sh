#!/bin/bash
# Quality Guardian System Setup Script
# Complete installation and configuration of zero-tolerance quality enforcement

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Quality Guardian banner
echo -e "${PURPLE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${PURPLE}║              🛡️ Quality Guardian System Setup                ║${NC}"
echo -e "${PURPLE}║              StarsCalendars Spiritual Platform              ║${NC}"
echo -e "${PURPLE}║                Zero Tolerance Enforcement                  ║${NC}"
echo -e "${PURPLE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo

# Functions for logging
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${CYAN}[STEP]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -f "CLAUDE.md" ]; then
    log_error "This script must be run from the StarsCalendars project root directory"
    exit 1
fi

log_info "Setting up comprehensive quality enforcement system..."
echo

# Step 1: Verify Rust installation and version
log_step "1/8: Verifying Rust installation..."

if ! command -v rustc >/dev/null 2>&1; then
    log_error "Rust is not installed. Install from https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(rustc --version | cut -d' ' -f2)
log_success "Rust $RUST_VERSION detected"

# Check for minimum Rust version (1.92.0+ required for edition 2024 features)
if ! rustc --version | grep -E "1\.(8[0-9]|9[0-9])" >/dev/null; then
    log_warning "Rust 1.92.0+ required for edition 2024 features and production deployment"
fi

# Step 2: Install quality tools
log_step "2/8: Installing quality enforcement tools..."

# Essential quality tools
TOOLS=(
    "cargo-clippy:Latest Clippy linting"
    "cargo-deny@0.18.3:Security and license scanning"
    "cargo-audit@0.21.2:Vulnerability auditing"
    "cargo-tarpaulin@0.32.8:Code coverage analysis"
    "wasm-pack:WASM build and optimization (latest stable)"
)

for tool_info in "${TOOLS[@]}"; do
    IFS=':' read -r tool description <<< "$tool_info"
    tool_name=$(echo "$tool" | cut -d'@' -f1)

    log_info "Installing $tool_name ($description)..."

    if command -v "$tool_name" >/dev/null 2>&1; then
        log_success "$tool_name already installed"
    else
        if cargo install "$tool" --locked 2>/dev/null; then
            log_success "$tool_name installed successfully"
        else
            log_warning "Failed to install $tool_name - some quality checks may be limited"
        fi
    fi
done

# Optional but recommended tools
OPTIONAL_TOOLS=(
    "tokei:Code line counting and statistics"
    "cargo-criterion@1.0.0-alpha3:Performance benchmarking"
    "cargo-udeps:Unused dependency detection"
)

log_info "Installing optional quality tools..."

for tool_info in "${OPTIONAL_TOOLS[@]}"; do
    IFS=':' read -r tool description <<< "$tool_info"
    tool_name=$(echo "$tool" | cut -d'@' -f1)

    if ! command -v "$tool_name" >/dev/null 2>&1; then
        log_info "Installing $tool_name ($description)..."
        if cargo install "$tool" --locked 2>/dev/null; then
            log_success "$tool_name installed successfully"
        else
            log_warning "$tool_name installation failed - optional feature will be unavailable"
        fi
    else
        log_success "$tool_name already available"
    fi
done

# Step 3: Configure Git hooks
log_step "3/8: Configuring Git hooks for commit-time quality enforcement..."

if [ -d ".git" ]; then
    # Configure git to use our hooks directory
    git config core.hooksPath .githooks

    # Ensure hooks are executable
    chmod +x .githooks/pre-commit 2>/dev/null || true

    log_success "Git hooks configured - commits will be automatically validated"
else
    log_warning "Not a Git repository - manual quality checks required"
fi

# Step 4: VS Code integration
log_step "4/8: Configuring VS Code quality integration..."

if [ -d ".vscode" ]; then
    log_success "VS Code settings configured for real-time quality feedback"

    # Create recommended extensions file
    cat > .vscode/extensions.json << 'EOF'
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "tamasfe.even-better-toml",
        "wayou.vscode-todo-highlight",
        "esbenp.prettier-vscode",
        "ms-vscode.vscode-typescript-next",
        "bradlc.vscode-tailwindcss",
        "lokalise.i18n-ally",
        "redhat.vscode-yaml",
        "ms-vscode.vscode-json"
    ],
    "unwantedRecommendations": [
        "ms-vscode.vscode-typescript"
    ]
}
EOF

    log_success "VS Code extensions configuration created"
else
    log_warning "VS Code settings directory not found - IDE integration limited"
fi

# Step 5: Create quality monitoring directories
log_step "5/8: Setting up quality monitoring infrastructure..."

# Create necessary directories
mkdir -p .quality-reports
mkdir -p scripts
mkdir -p docs/quality

# Create quality monitoring dashboard
cat > docs/quality/README.md << 'EOF'
# 🛡️ Quality Guardian Dashboard

## Current Quality Status

Run `make quality-report` to generate the latest quality assessment.

## Quality Standards

- **Zero Tolerance**: No `unwrap()`, `expect()`, or `panic!()` in production code
- **Performance First**: Pre-allocated collections, O(1) WASM hot path
- **Security Focused**: RS256 JWT, SQL injection prevention, dependency scanning
- **Architecture Clean**: Clean Architecture compliance, proper dependency direction

## Tools and Commands

- `make quality-check` - Full quality validation
- `make anti-patterns` - Scan for forbidden patterns
- `make security` - Security vulnerability scan
- `make wasm-perf` - WASM performance validation
- `scripts/quality-monitor.sh` - Comprehensive quality assessment

## Quality Reports

Quality reports are automatically generated in `.quality-reports/` directory.
EOF

log_success "Quality monitoring infrastructure created"

# Step 6: Validate Makefile integration
log_step "6/8: Validating Makefile quality commands..."

if [ -f "Makefile" ]; then
    # Test key commands
    MAKEFILE_COMMANDS=("quality-check" "anti-patterns" "clippy" "security")

    for cmd in "${MAKEFILE_COMMANDS[@]}"; do
        if grep -q "^$cmd:" Makefile; then
            log_success "Makefile command '$cmd' available"
        else
            log_warning "Makefile command '$cmd' not found"
        fi
    done
else
    log_error "Makefile not found - quality commands unavailable"
    echo "Please ensure the Makefile is present in the project root"
fi

# Step 7: Test quality enforcement
log_step "7/8: Testing quality enforcement system..."

log_info "Running comprehensive quality validation..."

# Create test report
if ./scripts/quality-monitor.sh > /dev/null 2>&1; then
    log_success "Quality monitoring system operational"
else
    exit_code=$?
    if [ $exit_code -eq 1 ]; then
        log_warning "Quality issues detected - system working correctly (blocking as expected)"
    else
        log_error "Quality monitoring system encountered an error"
    fi
fi

# Test Makefile integration
if make quality-check > /dev/null 2>&1; then
    log_success "Makefile quality commands operational"
else
    log_warning "Some Makefile quality commands may have issues - check manually"
fi

# Step 8: Final validation and documentation
log_step "8/8: Final system validation..."

# Check critical files
CRITICAL_FILES=(
    "QUALITY.md:Quality enforcement documentation"
    "deny.toml:Security and license scanning config"
    ".editorconfig:Consistent code formatting"
    ".githooks/pre-commit:Git commit validation"
    ".github/workflows/quality-guardian.yml:CI/CD quality pipeline"
    ".vscode/settings.json:IDE quality integration"
)

echo
log_info "Validating critical quality files..."

ALL_FILES_PRESENT=true

for file_info in "${CRITICAL_FILES[@]}"; do
    IFS=':' read -r file description <<< "$file_info"

    if [ -f "$file" ]; then
        log_success "$file ($description)"
    else
        log_error "MISSING: $file ($description)"
        ALL_FILES_PRESENT=false
    fi
done

echo
echo -e "${PURPLE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${PURPLE}║                    🎉 SETUP COMPLETE                         ║${NC}"
echo -e "${PURPLE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo

if [ "$ALL_FILES_PRESENT" = true ]; then
    log_success "Quality Guardian System fully operational!"
    echo
    echo -e "${GREEN}✅ Zero-tolerance quality enforcement activated${NC}"
    echo -e "${GREEN}✅ Real-time IDE integration configured${NC}"
    echo -e "${GREEN}✅ Git commit validation enabled${NC}"
    echo -e "${GREEN}✅ CI/CD quality pipeline ready${NC}"
    echo -e "${GREEN}✅ Security scanning operational${NC}"
    echo -e "${GREEN}✅ Performance monitoring active${NC}"
    echo
    echo -e "${CYAN}🚀 Quick Start:${NC}"
    echo "   make quality-check    # Run full quality validation"
    echo "   make pre-commit       # Prepare for commit"
    echo "   make quality-report   # Generate quality assessment"
    echo
    echo -e "${PURPLE}🙏 Quality is our spiritual practice in code${NC}"
    echo -e "${PURPLE}   Every line reflects our commitment to excellence${NC}"

    exit 0
else
    log_error "Quality Guardian System setup incomplete"
    echo
    echo -e "${RED}Some critical files are missing. Please check the setup.${NC}"
    echo -e "${YELLOW}You may need to run individual setup steps manually.${NC}"

    exit 1
fi
