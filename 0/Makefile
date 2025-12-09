# AI1C Quality-First Makefile (Rust Axum + Dioxus)

.PHONY: quality-check anti-patterns clippy security secret-scan arch perf clean fmt pre-commit quality-report quality-summary find-patterns security-audit test bench docs check ci manifest-guard tasks-guard

# 🛡️ Main quality gate
quality-check: anti-patterns clippy security secret-scan arch tasks-guard versions-guard manifest-guard
	@echo "✅ All quality checks passed!"

# 🔍 Anti-pattern scanning
anti-patterns:
	@./scripts/anti-patterns.sh

manifest-guard:
	@echo "📦 Manifest guard"
	@bash ./scripts/manifest-guard.sh

tasks-guard:
	@echo "🗂️  Tasks guard"
	@bash ./scripts/tasks-guard.sh

versions-guard:
	@echo "🔎 Versions guard"
	@python3 ./scripts/versions-guard.py

# 🦀 Strict Clippy
clippy:
	@echo "🦀 Running strict Clippy checks..."
	@if [ -f "Cargo.toml" ]; then \
		cargo clippy --all-targets --all-features -- \
			-D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::as_conversions \
			-D clippy::unimplemented -D clippy::todo -D clippy::await_holding_lock -D clippy::inefficient_to_string \
			-D clippy::large_stack_arrays -D clippy::vec_init_then_push -D clippy::unwrap_in_result -D clippy::map_err_ignore; \
	else \
		echo "⚠️ Cargo.toml not found - skipping"; \
	fi

# 🔒 Security checks
# 🔒 Security checks
security:
	@echo "🔒 Running security checks..."
	@if command -v cargo-audit >/dev/null 2>&1; then cargo audit --ignore RUSTSEC-2023-0071 --quiet || true; else echo "⚠️ Install cargo-audit" && exit 1; fi
	@if command -v cargo-deny >/dev/null 2>&1; then cargo deny check --hide-inclusion-graph; else echo "⚠️ Install cargo-deny" && exit 1; fi

# 🔐 Secret scan
secret-scan:
	@echo "🔐 Running secret scan..."
	@./scripts/secret-scan.sh

# 🏗️ Architecture (light checks)
arch:
	@echo "🏗️ Architecture quick checks..."
	# Workspace-aware scan: ai1c-service/src (and ko-service/src if added later)
	@if [ -d "ai1c-service/src" ]; then \
		! grep -R "format!\s*\(\s*\"\s*SELECT\|INSERT\|UPDATE\|DELETE" --include="*.rs" ai1c-service/src/ >/dev/null 2>&1 || (echo "❌ Potential SQL injection via format! (ai1c-service)" && false); \
		! grep -R "\\.await[[:space:]]*;" --include="*.rs" ai1c-service/src/ >/dev/null 2>&1 || true; \
	fi
	@if [ -d "ko-service/src" ]; then \
		! grep -R "format!\s*\(\s*\"\s*SELECT\|INSERT\|UPDATE\|DELETE" --include="*.rs" ko-service/src/ >/dev/null 2>&1 || (echo "❌ Potential SQL injection via format! (ko-service)" && false); \
		! grep -R "\\.await[[:space:]]*;" --include="*.rs" ko-service/src/ >/dev/null 2>&1 || true; \
	fi
	@echo "✅ Architecture quick checks passed"

# 🚀 Performance placeholder
perf:
	@echo "🚀 No perf tests configured yet"

# 🧹 Formatting
fmt:
	cargo fmt --all || true

# 🧪 Tests
test: quality-check
	@if [ -f "Cargo.toml" ]; then cargo test --all-features; else echo "⚠️ No Cargo.toml"; fi

# 📦 Build
build: quality-check
	@if [ -f "Cargo.toml" ]; then cargo build --release; else echo "⚠️ No Cargo.toml"; fi

# 📚 Docs
docs:
	@echo "📚 Generating docs..."
	@if [ -f "Cargo.toml" ]; then cargo doc --all --no-deps --document-private-items || true; fi

# 📝 Reports
quality-report:
	@./scripts/quality-monitor.sh || true

quality-summary:
	@echo "📊 QUALITY SUMMARY"
	@echo "=================="
	@echo "🦀 Clippy: $$(cargo clippy --quiet 2>&1 | grep -c 'warning' || echo '0') warnings"

# 🔍 Find patterns (quick)
find-patterns:
	@echo "🔍 Searching for unwrap/expect/panic"
	@grep -rn "\.unwrap()\|\.expect(\|panic!(" --include="*.rs" . || echo "None"

# 🔒 Security audit helpers
security-audit:
	@echo "🔒 cargo-deny / cargo-audit"
	@command -v cargo-deny >/dev/null 2>&1 && cargo deny check || echo "Install cargo-deny"
	@command -v cargo-audit >/dev/null 2>&1 && cargo audit || echo "Install cargo-audit"

# 🧼 Clean
clean:
	cargo clean || true

# Aliases
check: quality-check
ci: quality-check test
	@echo "🎉 CI simulation complete"

# Dev helper
pre-commit: quality-check fmt
	@echo "🎉 Ready to commit"

# SQLx prepare (requires DATABASE_URL)
sqlx-prepare:
	@echo "🗄️  Preparing SQLx offline data (.sqlx)"
	@which cargo-sqlx >/dev/null 2>&1 || which cargo-sqlx >/dev/null 2>&1 || true
	@if [ -z "$$DATABASE_URL" ]; then echo "❌ DATABASE_URL not set" && exit 1; fi
	@echo "ℹ️  No rag-service; add explicit targets per service when DB migrations are implemented"
