# fontdone Makefile
# ===========================
# Standalone entry point for build, test, lint, parity, and benchmark work.

CARGO := cargo
PYTHON := python3
PYCACHE_DIR := target/pycache
BENCH_SAMPLES ?= 10
BENCH_PROFILE ?= default
COVERAGE_TOOLCHAIN ?= nightly
CARGO_LLVM_COV_VERSION ?= 0.8.7
COVERAGE_UNIFIED_WORKERS ?= 1
# Opt-level 1 is the fastest measured current-host all-lane coverage profile;
# its coverage totals and parity results match the opt-level-3 comparison.
COVERAGE_TEST_OPT_LEVEL ?= 1
COVERAGE_TEST_DEBUG ?= 1
COVERAGE_LLVM_COV_FLAGS ?= --no-clean
# Current cargo-llvm-cov emits a report accepted directly by Coverage MCP.
# Keep the compatibility rewrite opt-in for older LLVM JSON producers.
COVERAGE_NORMALIZE_SEGMENTS ?= 0
COVERAGE_PREPARATION_JOBS ?= 2
COVERAGE_ALL_TARGET_DIR ?= target/llvm-cov-all-lanes
COVERAGE_TEST_BINARY ?=
COVERAGE_ABI_PREFLIGHT ?= 0
COVERAGE_UNIFIED_LANE_SPLIT ?= 1
CARGO_DENY_VERSION ?= 0.20.2
CARGO_AUDIT_VERSION ?= 0.22.2
PREFIX ?= /usr/local
DESTDIR ?=
PARITY_ARGS ?= -- --nocapture
COVERAGE_OUTPUT ?= target/coverage/unified-summary.json
ALL_LANES_COVERAGE_OUTPUT ?= target/coverage/unified-runtime-all-lanes.json
CONDITION_COVERAGE_OUTPUT ?= target/coverage/unified-condition-summary.json
CONDITION_COVERAGE_LINES_OUTPUT ?= target/coverage/unified-condition-missing-lines.txt
CONDITION_COVERAGE_NORMALIZED_OUTPUT ?= target/coverage/unified-condition-summary-normalized.json
CORE_COVERAGE_IGNORE_REGEX := /(fontdone-c-abi|fontdone-wasm)/src/
ALL_LANES_COVERAGE_IGNORE_REGEX := /tests/
PLATFORM_TARGET ?=
PLATFORM_CC ?=
PLATFORM_NM ?=
PLATFORM_RUNNER ?=
PLATFORM_SYSROOT ?=
PLATFORM_CLANG_TARGET ?= $(PLATFORM_TARGET)

.DEFAULT_GOAL := help
.NOTPARALLEL: ci ci-fast ci-commit ci-thorough release-verify

.PHONY: help
help:
	@printf "fontdone\n\n"
	@printf "Start:\n"
	@printf "  make setup            Build the pinned C oracle and public constants\n"
	@printf "  make build            Build fontdone\n"
	@printf "  make test-fast        Run tests that do not need the C oracle\n"
	@printf "  make test-parity-smoke Run a small exact C/Rust/C-ABI/WASM runtime smoke matrix\n"
	@printf "  make test-parity      Run the complete exact parity gate\n"
	@printf "  make lint             Check formatting and Clippy\n"
	@printf "  make ci-fast          Run the exact fast per-commit local CI gate\n"
	@printf "  make ci               Alias for make ci-fast\n"
	@printf "  make ci-thorough      Run the requested pre-merge local gate\n"
	@printf "\nFocused work:\n"
	@printf "  make test-op OP=<op>      Test one public operation\n"
	@printf "  make test-case CASE=<id>  Test one case or subject\n"
	@printf "  make test-opentype-validator  Verify OpenType validation and face-memory ownership\n"
	@printf "  make test-gx-validator        Verify GX and classic-kern validation ownership\n"
	@printf "  make test-exact-errors [CASE=<id>]  Force exact C error/output comparison for all or one runnable error case\n"
	@printf "  make audit-exact-errors [CASE=<id>] Report exact error/output debt without failing the full audit\n"
	@printf "  make api-abi-audit        Generate the public API/ABI audit\n"
	@printf "  make api-abi-check        Check API/ABI routing and wrappers\n"
	@printf "  make external-c-abi-audit Trace real C calls against the pinned oracle\n"
	@printf "  make optional-feature-contract  Prove optional build branches and LCD state in four lanes\n"
	@printf "  make check-c-contract-inventory  Validate fixed ownership/state/module/artifact/platform denominators\n"
	@printf "  make platform-contract     Record this native target's hash-bound C/layout evidence\n"
	@printf "  make platform-contract-cross PLATFORM_TARGET=... PLATFORM_CC=... PLATFORM_NM=... PLATFORM_RUNNER='...' PLATFORM_SYSROOT=...  Record an emulated Linux target\n"
	@printf "  make check-platform-contract  Require five fresh target bundles plus Windows import-library evidence\n"
	@printf "  make c-abi-contract       Report all pinned C contract categories\n"
	@printf "  make c-abi-contract-all-platforms  Validate five bundles and report current C-contract debt\n"
	@printf "  make c-abi-contract-complete  Require all 12 C contract categories from assembled evidence\n"
	@printf "  make test-coverage        Write core Rust coverage JSON\n"
	@printf "  make test-coverage-all    Write all-lane branch coverage JSON\n"
	@printf "  make coverage-clean       Remove cached LLVM coverage build artifacts\n"
	@printf "  make bench-quick          Run the benchmark smoke gate\n"
	@printf "  make bench-regression     Require the reviewed performance thresholds\n"
	@printf "  make record-performance-baseline  Commit-ready evidence from the latest qualifying clean run\n"
	@printf "\nGenerated inputs:\n"
	@printf "  make font-fixtures        Rebuild fixture inputs and payloads\n"
	@printf "  make check-font-fixtures  Reject fixture generator drift\n"
	@printf "  make check-generated      Reject generated contract drift\n"
	@printf "  make repository-inventory Refresh the reviewed file-retention ledger\n"
	@printf "\nDocumentation:\n"
	@printf "  make check-docs           Validate every tracked guide and rustdoc policy\n"
	@printf "  make doc                  Build strict workspace API documentation\n"
	@printf "  make doc-test             Compile every public Rust example\n"
	@printf "  make record-parity-snapshot  Commit-ready snapshot from the latest source-matched full run\n"
	@printf "\nRelease:\n"
	@printf "  make release-verify       Run the complete local release gate\n"
	@printf "  make release-dry-run      Verify publishable archives\n"
	@printf "  make release              Publish after protected approval\n"
	@printf "  make c-abi-install        Install C headers, libraries, and pkg-config metadata under PREFIX\n"
	@printf "  make c-abi-install-check  Stage and verify the complete C installation layout\n"
	@printf "\nMaintenance:\n"
	@printf "  make setup-tools          Install pinned supply-chain audit tools\n"
	@printf "  make setup-coverage-tools Install the pinned cargo-llvm-cov version\n"
	@printf "  make supply-chain         Run dependency and license audits\n"
	@printf "  make clean                Remove generated local artifacts\n"

.PHONY: setup
setup: unified-oracle public-constants

.PHONY: setup-tools
setup-tools:
	$(CARGO) install cargo-deny --version $(CARGO_DENY_VERSION) --locked
	$(CARGO) install cargo-audit --version $(CARGO_AUDIT_VERSION) --locked

.PHONY: setup-coverage-tools
setup-coverage-tools:
	$(CARGO) install cargo-llvm-cov --version $(CARGO_LLVM_COV_VERSION) --locked

.PHONY: build
build:
	$(CARGO) build --locked

.PHONY: doc
doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --all-features --no-deps --locked

.PHONY: doc-test
doc-test:
	RUSTDOCFLAGS="-D warnings" $(CARGO) test --workspace --all-features --doc --locked

.PHONY: test-fast
test-fast:
	$(CARGO) test --workspace --all-features --locked -- --skip unified_fixture_parity --skip pipe_trace
	$(CARGO) check --workspace --all-targets --all-features --locked

.PHONY: fresh-checkout-check
fresh-checkout-check:
	$(CARGO) --version
	rustc --version
	$(PYTHON) --version
	$(CARGO) build --workspace --all-features --locked
	$(MAKE) test-fast
	$(MAKE) doc
	$(MAKE) doc-test
	$(PYTHON) scripts/generate_support_matrix.py --check
	$(PYTHON) scripts/generate_wasm_contract.py --check
	$(PYTHON) scripts/sync_package_licenses.py --check
	$(MAKE) check-docs
	$(MAKE) check-versions

.PHONY: test-parity
test-parity: unified-oracle bzip2-enabled-oracle api-abi-check test-ffi test-filter-guard
	$(PYTHON) scripts/run_runtime_parity.py

.PHONY: test-parity-smoke
test-parity-smoke: unified-oracle api-abi-audit test-ffi
	FONTDONE_UNIFIED_OPERATION_FILTER=load_char \
	FONTDONE_UNIFIED_CASE_LIMIT=8 \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: record-parity-snapshot
record-parity-snapshot:
	$(PYTHON) scripts/run_runtime_parity.py --record

.PHONY: unified-oracle
unified-oracle: oracle-fetch
	bash scripts/build_ft.sh
	$(PYTHON) scripts/build_unified_oracle.py

.PHONY: lzw-disabled-oracle
lzw-disabled-oracle: oracle-fetch
	$(PYTHON) scripts/build_unified_oracle.py --variant lzw-disabled

.PHONY: color-layers-disabled-oracle
color-layers-disabled-oracle: oracle-fetch
	$(PYTHON) scripts/build_unified_oracle.py --variant color-layers-disabled

.PHONY: subpixel-rendering-enabled-oracle
subpixel-rendering-enabled-oracle: oracle-fetch
	$(PYTHON) scripts/build_unified_oracle.py --variant subpixel-rendering-enabled

.PHONY: bzip2-enabled-oracle
bzip2-enabled-oracle: oracle-fetch font-fixture-compressed
	$(PYTHON) scripts/build_unified_oracle.py --variant bzip2-enabled

.PHONY: optional-feature-contract
optional-feature-contract: api-abi-audit unified-oracle lzw-disabled-oracle color-layers-disabled-oracle subpixel-rendering-enabled-oracle
	CARGO_TARGET_DIR=target/optional-features-disabled \
	$(CARGO) build --release -p fontdone-c-abi --lib \
		--example optional_feature_probe --no-default-features --locked
	$(PYTHON) scripts/build_external_c_oracle.py \
		--cargo-target-dir target/optional-features-disabled \
		--constants-dir target/unified-fixtures \
		--verify-optional-features-disabled \
		--out target/optional-features-disabled/unified-fixtures/gen_fontdone_external
	CARGO_TARGET_DIR=target/subpixel-rendering-enabled \
	$(CARGO) build --release -p fontdone-c-abi --lib \
		--example optional_feature_probe --no-default-features \
		--features subpixel-rendering --locked
	$(PYTHON) scripts/build_external_c_oracle.py \
		--cargo-target-dir target/subpixel-rendering-enabled \
		--constants-dir target/unified-fixtures \
		--verify-subpixel-rendering-enabled \
		--out target/subpixel-rendering-enabled/unified-fixtures/gen_fontdone_external

.PHONY: external-c-oracle
external-c-oracle: unified-oracle bzip2-enabled-oracle api-abi-check
	$(CARGO) build --release -p fontdone-c-abi --locked
	$(PYTHON) scripts/build_external_c_oracle.py

.PHONY: external-c-abi-audit
external-c-abi-audit: external-c-oracle
	FONTDONE_EXTERNAL_C_AUDIT=1 \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: public-constants
public-constants: oracle-fetch
	$(PYTHON) scripts/generate_public_constants.py --rust-src src/ffi/generated_constants.rs --test-lookup tests/support/generated_constant_lookup.rs

.PHONY: generate-contracts
generate-contracts: oracle-fetch
	$(PYTHON) scripts/generate_support_matrix.py
	$(PYTHON) scripts/generate_wasm_contract.py
	$(PYTHON) scripts/generate_c_contract_macros.py
	$(PYTHON) scripts/generate_c_contract_headers.py
	$(PYTHON) scripts/sync_package_licenses.py
	$(PYTHON) scripts/audit_repository_files.py

.PHONY: check-generated
check-generated: oracle-fetch
	$(PYTHON) scripts/generate_public_constants.py --check --rust-src src/ffi/generated_constants.rs --test-lookup tests/support/generated_constant_lookup.rs
	$(PYTHON) scripts/generate_support_matrix.py --check
	$(PYTHON) scripts/generate_wasm_contract.py --check
	$(PYTHON) scripts/generate_c_contract_macros.py --check
	$(PYTHON) scripts/generate_c_contract_headers.py --check
	$(PYTHON) scripts/sync_package_licenses.py --check
	$(PYTHON) scripts/audit_repository_files.py --check

.PHONY: check-docs
check-docs:
	$(PYTHON) scripts/check_documentation.py
	$(PYTHON) scripts/check_rustdoc_contracts.py

.PHONY: test-coverage
test-coverage: unified-oracle api-abi-runtime-check
	mkdir -p $(dir $(COVERAGE_OUTPUT))
	$(CARGO) llvm-cov clean --profraw-only
	CARGO_PROFILE_TEST_OPT_LEVEL=$(COVERAGE_TEST_OPT_LEVEL) \
	CARGO_PROFILE_TEST_DEBUG=$(COVERAGE_TEST_DEBUG) \
	$(CARGO) llvm-cov --test unified_fixture_parity --locked --json \
		$(COVERAGE_LLVM_COV_FLAGS) \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		--output-path $(COVERAGE_OUTPUT) -- unified_fixture_parity --nocapture

.PHONY: test-coverage-all
test-coverage-all:
	+$(MAKE) --no-print-directory -j$(COVERAGE_PREPARATION_JOBS) \
		unified-oracle api-abi-runtime-check \
		$(if $(filter 1,$(COVERAGE_ABI_PREFLIGHT)),coverage-abi-preflight)
	mkdir -p $(dir $(ALL_LANES_COVERAGE_OUTPUT))
	CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) $(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov clean --profraw-only
ifeq ($(COVERAGE_UNIFIED_LANE_SPLIT),1)
# Build one instrumented integration binary, then run the Rust FFI, C ABI, and
# host-WASM comparisons in separate processes. LLVM profile counters are
# process-local, so this avoids the lock/atomic contention seen when those
# backends run concurrently in one instrumented process. cargo-llvm-cov merges
# the three raw profiles in the report step below.
	CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) \
	CARGO_PROFILE_TEST_OPT_LEVEL=$(COVERAGE_TEST_OPT_LEVEL) \
	CARGO_PROFILE_TEST_DEBUG=$(COVERAGE_TEST_DEBUG) \
	$(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov --branch --workspace \
		--test unified_fixture_parity --exclude-from-test fontdone-c-abi \
		--exclude-from-test fontdone-wasm --locked \
		$(filter-out --no-clean,$(COVERAGE_LLVM_COV_FLAGS)) --no-run \
		--ignore-filename-regex '$(ALL_LANES_COVERAGE_IGNORE_REGEX)' \
		--output-path $(COVERAGE_ALL_TARGET_DIR)/coverage-build.json \
		-- unified_fixture_parity --nocapture
	@set -u; \
	test_binary="$(COVERAGE_TEST_BINARY)"; \
	if [ -z "$$test_binary" ]; then \
	  test_binary=$$(find $(COVERAGE_ALL_TARGET_DIR)/llvm-cov-target/debug/deps \
	    -maxdepth 1 -type f -name 'unified_fixture_parity-*' -perm -111 -print \
	    | xargs ls -dt 2>/dev/null | head -n 1); \
	fi; \
	if [ ! -x "$$test_binary" ]; then \
	  echo "coverage test binary not found under $(COVERAGE_ALL_TARGET_DIR)/llvm-cov-target/debug/deps" >&2; \
	  exit 1; \
	fi; \
	lane_status=0; \
	( CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) \
	  FONTDONE_UNIFIED_WORKERS=$(COVERAGE_UNIFIED_WORKERS) \
	  FONTDONE_UNIFIED_BACKEND=rust \
	  LLVM_PROFILE_FILE=$(COVERAGE_ALL_TARGET_DIR)/llvm-cov-target/fontdone-rust-%p-%m.profraw \
	  "$$test_binary" unified_fixture_parity --exact --nocapture ) & rust_pid=$$!; \
	( CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) \
	  FONTDONE_UNIFIED_WORKERS=$(COVERAGE_UNIFIED_WORKERS) \
	  FONTDONE_UNIFIED_BACKEND=c-abi \
	  LLVM_PROFILE_FILE=$(COVERAGE_ALL_TARGET_DIR)/llvm-cov-target/fontdone-c-abi-%p-%m.profraw \
	  "$$test_binary" unified_fixture_parity --exact --nocapture ) & c_abi_pid=$$!; \
	( CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) \
	  FONTDONE_UNIFIED_WORKERS=$(COVERAGE_UNIFIED_WORKERS) \
	  FONTDONE_UNIFIED_BACKEND=wasm \
	  LLVM_PROFILE_FILE=$(COVERAGE_ALL_TARGET_DIR)/llvm-cov-target/fontdone-wasm-%p-%m.profraw \
	  "$$test_binary" unified_fixture_parity --exact --nocapture ) & wasm_pid=$$!; \
	wait $$rust_pid || lane_status=1; \
	wait $$c_abi_pid || lane_status=1; \
	wait $$wasm_pid || lane_status=1; \
	exit $$lane_status
	CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) \
	$(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov report \
		--package fontdone --package fontdone-c-abi --package fontdone-wasm \
		--locked --json \
		--ignore-filename-regex '$(ALL_LANES_COVERAGE_IGNORE_REGEX)' \
		--output-path $(ALL_LANES_COVERAGE_OUTPUT)
else
# Keep workspace report scope for the C-ABI and WASM facades, but execute
# only the integration binary that drives all three parity lanes. Running
# the workspace's empty unit and pipe-trace targets duplicates cfg-dependent
# FFI coverage without adding a parity input.
	CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) \
	CARGO_PROFILE_TEST_OPT_LEVEL=$(COVERAGE_TEST_OPT_LEVEL) \
	CARGO_PROFILE_TEST_DEBUG=$(COVERAGE_TEST_DEBUG) \
	FONTDONE_UNIFIED_WORKERS=$(COVERAGE_UNIFIED_WORKERS) \
	$(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov --branch --workspace \
		--test unified_fixture_parity \
		$(COVERAGE_LLVM_COV_FLAGS) \
		--exclude-from-test fontdone-c-abi \
		--exclude-from-test fontdone-wasm \
		--locked --json \
		--ignore-filename-regex '$(ALL_LANES_COVERAGE_IGNORE_REGEX)' \
		--output-path $(ALL_LANES_COVERAGE_OUTPUT) -- --nocapture
endif

ifeq ($(COVERAGE_NORMALIZE_SEGMENTS),1)
	jq -c '(.data[]?.files[]?.segments[]? | select(length >= 3) | .[2]) |= if . > 2147483647 then 2147483647 else . end' \
		$(ALL_LANES_COVERAGE_OUTPUT) > $(ALL_LANES_COVERAGE_OUTPUT).tmp
	mv $(ALL_LANES_COVERAGE_OUTPUT).tmp $(ALL_LANES_COVERAGE_OUTPUT)
endif

.PHONY: coverage-abi-preflight
coverage-abi-preflight:
	$(CARGO) test -p fontdone-c-abi -p fontdone-wasm --lib --features abi-test-support --locked

.PHONY: coverage-clean
coverage-clean:
	$(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov clean --workspace
	CARGO_TARGET_DIR=$(COVERAGE_ALL_TARGET_DIR) $(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov clean --workspace

.PHONY: test-unified-condition-coverage
test-unified-condition-coverage: unified-oracle api-abi-runtime-check
	mkdir -p $(dir $(CONDITION_COVERAGE_OUTPUT))
	FONTDONE_ENABLE_SILENT_TRACE_LOGGER=1 \
	RUSTFLAGS="-Zcoverage-options=condition" $(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov \
		--test unified_fixture_parity --locked --json \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		--output-path $(CONDITION_COVERAGE_OUTPUT) -- unified_fixture_parity --nocapture
	RUSTFLAGS="-Zcoverage-options=condition" $(CARGO) +$(COVERAGE_TOOLCHAIN) llvm-cov report \
		--show-missing-lines \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		> $(CONDITION_COVERAGE_LINES_OUTPUT)

.PHONY: normalize-unified-condition-coverage
normalize-unified-condition-coverage:
	mkdir -p $(dir $(CONDITION_COVERAGE_NORMALIZED_OUTPUT))
	jq -c '(.data[]?.files[]?.segments[]? | select(length >= 3) | .[2]) |= if . > 2147483647 then 2147483647 else . end' \
		$(CONDITION_COVERAGE_OUTPUT) > $(CONDITION_COVERAGE_NORMALIZED_OUTPUT).tmp
	mv $(CONDITION_COVERAGE_NORMALIZED_OUTPUT).tmp $(CONDITION_COVERAGE_NORMALIZED_OUTPUT)

.PHONY: test-ffi
test-ffi:
# Core rendering code must not pull in native FreeType, bindgen, or system
# FFI libraries.  extern "C" in src/ffi/ is expected (thin facade types).
	@! grep -En 'freetype-sys|^bindgen |^cc |dlopen|libloading|pkg-config' Cargo.toml
	@for f in $$(find src -name '*.rs' ! -path 'src/ffi/*'); do \
		if grep -Eqn 'freetype-sys|bindgen|dlopen|libloading|pkg-config' "$$f"; then \
			echo "$$f: forbidden FFI dependency" >&2; exit 1; \
		fi; \
	done
	@echo "no-runtime-FFI guard: clean"

# ── Single Feature Testing ────────────────────────────────────────────
# These targets run the unified parity test filtered to a single operation
# or case. They always refresh the oracle cache (FONTDONE_UNIFIED_ORACLE_REFRESH=1)
# to avoid cache-key confusion when switching between filters.
#
# Usage:
#   make test-op OP=ftadvanc.get_advance
#   make test-case CASE=load_glyph
#   make test-list
#
# The full-suite cache is never affected — different filter selections produce
# different cache keys, so filtered runs and full-suite runs use separate cache
# files under tests/fixtures/outputs/unified_oracle_cache/.
#
# Env vars (all optional):
#   FONTDONE_UNIFIED_OPERATION_FILTER  – substring match on operation name
#   FONTDONE_UNIFIED_CASE_FILTER       – substring match on case_id/subject/case
#   FONTDONE_UNIFIED_CASE_LIMIT        – max number of selected concrete cases
#   FONTDONE_UNIFIED_WORKERS           – bounded backend comparison worker count
#   COVERAGE_TEST_OPT_LEVEL             – optimization level for coverage builds
#   COVERAGE_TEST_DEBUG                 – line-table debug level for coverage builds
#   COVERAGE_NORMALIZE_SEGMENTS          – rewrite oversized LLVM segment counts (1/0)
#   COVERAGE_LLVM_COV_FLAGS             – extra cargo-llvm-cov flags for coverage builds
#   COVERAGE_PREPARATION_JOBS           – parallel jobs for independent coverage setup
#   COVERAGE_ALL_TARGET_DIR             – isolated cached target for all-lane LLVM coverage
#   COVERAGE_TEST_BINARY                – optional instrumented test binary override; otherwise the newest built binary is used
#   COVERAGE_ABI_PREFLIGHT               – rerun the standalone ABI unit preflight (1/0)
#   COVERAGE_UNIFIED_LANE_SPLIT          – run Rust, C ABI, and WASM coverage lanes as separate processes (1/0)
#   FONTDONE_UNIFIED_BACKEND             – internal lane selector: rust, c-abi, or wasm
#   FONTDONE_UNIFIED_ORACLE_REFRESH    – force skip cache, re-run C oracle
#   FONTDONE_UNIFIED_SELECTION_ONLY    – print selection, don't execute
#   FONTDONE_UNIFIED_PROFILE           – print timing profiles
#   FONTDONE_UNIFIED_EXPECTED_ERROR_ONLY – select only expect_error cases
#   FONTDONE_UNIFIED_STRICT_EXPECTED_ERRORS – force exact error/output comparison
#   FONTDONE_UNIFIED_REPORT_EXPECTED_ERROR_MISMATCHES – write strict ledger without failing

.PHONY: test-op
test-op: unified-oracle api-abi-check
	@if [ -z "$(OP)" ]; then \
		echo "Usage: make test-op OP=<operation>" >&2; \
		echo "  Example operations: ftadvanc.get_advance, load_glyph, render_glyph, freetype.inspect_glyph_metrics" >&2; \
		exit 1; \
	fi
	FONTDONE_UNIFIED_OPERATION_FILTER="$(OP)" \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-case
test-case: unified-oracle api-abi-check
	@if [ -z "$(CASE)" ]; then \
		echo "Usage: make test-case CASE=<substring>" >&2; \
		echo "  Matches against case_id, subject, or case fields" >&2; \
		exit 1; \
	fi
	FONTDONE_UNIFIED_CASE_FILTER="$(CASE)" \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-opentype-validator
test-opentype-validator: font-fixture-sfnt external-c-oracle
	FONTDONE_UNIFIED_CASE_FILTER="ftotval." \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture
	FONTDONE_EXTERNAL_C_AUDIT=1 \
	FONTDONE_EXTERNAL_C_AUDIT_FOCUSED=1 \
	FONTDONE_UNIFIED_CASE_FILTER="ftotval." \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-gx-validator
test-gx-validator: font-fixture-gxvalid external-c-oracle
	FONTDONE_UNIFIED_OPERATION_FILTER="ftgxval." \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture
	FONTDONE_EXTERNAL_C_AUDIT=1 \
	FONTDONE_EXTERNAL_C_AUDIT_FOCUSED=1 \
	FONTDONE_UNIFIED_OPERATION_FILTER="ftgxval." \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-exact-errors
test-exact-errors: unified-oracle api-abi-check
	FONTDONE_UNIFIED_CASE_FILTER="$(CASE)" \
	FONTDONE_UNIFIED_EXPECTED_ERROR_ONLY=1 \
	FONTDONE_UNIFIED_STRICT_EXPECTED_ERRORS=1 \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: audit-exact-errors
audit-exact-errors: unified-oracle api-abi-check
	FONTDONE_UNIFIED_CASE_FILTER="$(CASE)" \
	FONTDONE_UNIFIED_EXPECTED_ERROR_ONLY=1 \
	FONTDONE_UNIFIED_STRICT_EXPECTED_ERRORS=1 \
	FONTDONE_UNIFIED_REPORT_EXPECTED_ERROR_MISMATCHES=1 \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-pending-case
test-pending-case: unified-oracle api-abi-check
	@if [ -z "$(CASE)" ]; then \
		echo "Usage: make test-pending-case CASE=<exact-case-id>" >&2; \
		exit 2; \
	fi
	FONTDONE_UNIFIED_CASE_FILTER="$(CASE)" \
	FONTDONE_UNIFIED_INCLUDE_PENDING_CASE="$(CASE)" \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-filter-guard
test-filter-guard: unified-oracle api-abi-check
	@output="$$(mktemp)"; \
	trap 'rm -f "$$output"' EXIT; \
	if FONTDONE_UNIFIED_CASE_FILTER="__fontdone_no_matching_fixture__" \
		FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
		$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture \
		>"$$output" 2>&1; then \
		cat "$$output"; \
		echo "expected an unmatched explicit fixture filter to fail" >&2; \
		exit 1; \
	fi; \
	if ! grep -Fq "explicit runtime filter matched no fixture cases" "$$output"; then \
		cat "$$output"; \
		echo "unmatched fixture filter failed for an unexpected reason" >&2; \
		exit 1; \
	fi

.PHONY: test-list
test-list: unified-oracle api-abi-check
	FONTDONE_UNIFIED_SELECTION_ONLY=1 \
	FONTDONE_UNIFIED_CASE_LIMIT=5 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-pipe-trace
test-pipe-trace:
	$(CARGO) test --test pipe_trace --locked -- --ignored --nocapture

.PHONY: api-abi-audit
api-abi-audit: oracle-fetch
	$(PYTHON) scripts/generate_c_contract_macros.py --check
	$(PYTHON) scripts/generate_c_contract_headers.py --check
	$(PYTHON) scripts/audit_api_abi.py

.PHONY: api-abi-check
api-abi-check: api-abi-audit optional-feature-contract
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json --route-audit

.PHONY: api-abi-runtime-check
api-abi-runtime-check: api-abi-audit
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json --route-audit

.PHONY: c-abi-contract
c-abi-contract: check-c-contract-inventory test-c-abi-safety external-c-abi-audit audit-exact-errors platform-contract
	$(PYTHON) scripts/audit_api_abi.py --c-contract

.PHONY: c-abi-contract-complete
c-abi-contract-complete: check-c-contract-inventory test-c-abi-safety \
	external-c-abi-audit audit-exact-errors test-c-consumer check-c-exports \
	check-platform-contract
	$(PYTHON) scripts/audit_api_abi.py --c-contract --require-c-contract-complete

.PHONY: c-abi-contract-all-platforms
c-abi-contract-all-platforms: check-c-contract-inventory test-c-abi-safety \
	external-c-abi-audit audit-exact-errors test-c-consumer check-c-exports \
	check-platform-contract
	$(PYTHON) scripts/audit_api_abi.py --c-contract

.PHONY: test-c-abi-safety
test-c-abi-safety:
	$(CARGO) test -p fontdone-c-abi --lib --all-features --locked

.PHONY: check-c-contract-inventory
check-c-contract-inventory: api-abi-check
	$(PYTHON) scripts/audit_api_abi.py --check-contract-inventory

.PHONY: platform-contract
platform-contract: api-abi-audit test-c-consumer check-c-exports
	$(PYTHON) scripts/audit_api_abi.py --record-platform-contract

.PHONY: platform-contract-cross
platform-contract-cross: oracle-fetch
	@test -n "$(PLATFORM_TARGET)" || { echo "PLATFORM_TARGET is required" >&2; exit 2; }
	@test -n "$(PLATFORM_CC)" || { echo "PLATFORM_CC is required" >&2; exit 2; }
	@test -n "$(PLATFORM_NM)" || { echo "PLATFORM_NM is required" >&2; exit 2; }
	@test -n "$(PLATFORM_RUNNER)" || { echo "PLATFORM_RUNNER is required" >&2; exit 2; }
	@test -d "$(PLATFORM_SYSROOT)" || { echo "PLATFORM_SYSROOT must be an existing directory" >&2; exit 2; }
	$(PYTHON) scripts/generate_c_contract_macros.py --check
	$(PYTHON) scripts/generate_c_contract_headers.py --check
	$(PYTHON) scripts/test_c_consumer.py \
		--target "$(PLATFORM_TARGET)" \
		--cc "$(PLATFORM_CC)" \
		--runner "$(PLATFORM_RUNNER)"
	$(PYTHON) scripts/check_c_exports.py \
		--target "$(PLATFORM_TARGET)" \
		--nm "$(PLATFORM_NM)"
	$(PYTHON) scripts/audit_api_abi.py \
		--record-platform-contract \
		--platform-target "$(PLATFORM_TARGET)" \
		--platform-runner "$(PLATFORM_RUNNER)" \
		--platform-linker "$(PLATFORM_CC)" \
		--platform-clang-target "$(PLATFORM_CLANG_TARGET)" \
		--platform-clang-sysroot "$(PLATFORM_SYSROOT)"

.PHONY: check-platform-contract
check-platform-contract: api-abi-audit
	$(PYTHON) scripts/audit_api_abi.py --check-platform-contract

.PHONY: route-buckets
route-buckets: api-abi-check
	$(PYTHON) scripts/report_pending_route_buckets.py

.PHONY: font-fixture-hinter
font-fixture-hinter:
	$(PYTHON) -m fontTools.ttx -q --no-recalc-timestamp -o tests/fixtures/input/fonts/glyf/hinter-control-matrix.ttf \
		tests/fixtures/input/font-sources/hinter-control-matrix.ttx
	$(PYTHON) scripts/font_generation/build_hinter_edge_fixtures.py

.PHONY: font-fixture-render
font-fixture-render:
	$(PYTHON) scripts/font_generation/build_render_fixtures.py

.PHONY: font-fixture-cjk
font-fixture-cjk:
	$(PYTHON) -m fontTools.ttx -q --no-recalc-timestamp -o tests/fixtures/input/fonts/autohint/cjk-coverage.ttf \
		tests/fixtures/input/font-sources/cjk-coverage.ttx
	$(PYTHON) -m fontTools.ttx -q --no-recalc-timestamp -o tests/fixtures/input/fonts/autohint/cjk-width-order.ttf \
		tests/fixtures/input/font-sources/cjk-width-order.ttx

.PHONY: font-fixture-autohint-scripts
font-fixture-autohint-scripts:
	$(PYTHON) scripts/font_generation/build_autohint_script_fixtures.py

.PHONY: font-fixture-cff
font-fixture-cff:
	$(PYTHON) scripts/font_generation/build_cff_fixtures.py

.PHONY: font-fixture-type1
font-fixture-type1:
	$(PYTHON) scripts/font_generation/build_type1_fixtures.py

.PHONY: font-fixture-type42
font-fixture-type42:
	$(PYTHON) scripts/font_generation/build_type42_fixtures.py

.PHONY: font-fixture-winfnt
font-fixture-winfnt:
	$(PYTHON) scripts/font_generation/generate_winfnt_fixtures.py

.PHONY: font-fixture-gasp
font-fixture-gasp: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_gasp_fixtures.py

.PHONY: font-fixture-compressed
font-fixture-compressed:
	$(PYTHON) scripts/build_compressed_fixtures.py

.PHONY: font-fixture-metrics
font-fixture-metrics: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_metric_fixtures.py

.PHONY: font-fixture-mvar
font-fixture-mvar:
	$(PYTHON) scripts/font_generation/build_mvar_fixtures.py

.PHONY: font-fixture-cmap
font-fixture-cmap: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_cmap_fixtures.py

.PHONY: font-fixture-color
font-fixture-color:
	$(PYTHON) scripts/font_generation/build_cpal_palette_fixtures.py

.PHONY: font-fixture-post
font-fixture-post: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_post_fixtures.py

.PHONY: font-fixture-fvar
font-fixture-fvar:
	$(PYTHON) scripts/font_generation/build_fvar_fixtures.py

.PHONY: font-fixture-ftmm-future
font-fixture-ftmm-future:
	$(PYTHON) scripts/font_generation/build_ftmm_future_variable_fixtures.py

.PHONY: font-fixture-name
font-fixture-name:
	$(PYTHON) scripts/font_generation/build_name_fixtures.py

.PHONY: font-fixture-sfnt
font-fixture-sfnt: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_sfnt_fixtures.py

.PHONY: font-fixture-gxvalid
font-fixture-gxvalid: font-fixture-hinter font-fixture-type1
	$(PYTHON) scripts/font_generation/build_gxvalid_fixtures.py

.PHONY: font-fixture-sbit
font-fixture-sbit: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_sbit_fixtures.py

.PHONY: font-fixture-sbix
font-fixture-sbix: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_sbix_fixtures.py

.PHONY: font-fixture-interpreter-version
font-fixture-interpreter-version:
	$(PYTHON) scripts/font_generation/build_interpreter_version_fixtures.py

.PHONY: font-fixture-pcf
font-fixture-pcf:
	$(PYTHON) scripts/font_generation/build_pcf_fixtures.py

.PHONY: font-fixture-pfr
font-fixture-pfr:
	$(PYTHON) scripts/font_generation/build_pfr_fixtures.py

.PHONY: font-fixture-svg
font-fixture-svg: font-fixture-hinter
	$(PYTHON) scripts/font_generation/build_svg_fixtures.py

.PHONY: font-fixture-malformed-bdf
font-fixture-malformed-bdf:
	$(PYTHON) scripts/font_generation/generate_malformed_bdf_fixtures.py

.PHONY: font-fixtures
font-fixtures: font-fixture-autohint-scripts font-fixture-cff font-fixture-type1
font-fixtures: font-fixture-type42
font-fixtures: font-fixture-winfnt font-fixture-gasp font-fixture-metrics
font-fixtures: font-fixture-mvar font-fixture-cmap font-fixture-color
font-fixtures: font-fixture-post font-fixture-fvar font-fixture-ftmm-future
font-fixtures: font-fixture-name font-fixture-sfnt font-fixture-sbit font-fixture-pcf font-fixture-pfr font-fixture-svg
font-fixtures: font-fixture-sbix
font-fixtures: font-fixture-interpreter-version
font-fixtures: font-fixture-gxvalid
font-fixtures: font-fixture-malformed-bdf font-fixture-render font-fixture-cjk
font-fixtures: font-fixture-compressed

.PHONY: check-font-fixtures
check-font-fixtures: font-fixtures
	git diff --exit-code -- tests/fixtures/input tests/fixtures/compressed
	$(PYTHON) scripts/audit_repository_files.py --check

.PHONY: repository-inventory
repository-inventory:
	$(PYTHON) scripts/audit_repository_files.py

.PHONY: fmt
fmt:
	$(CARGO) fmt --all -- --check

.PHONY: fmt-fix
fmt-fix:
	$(CARGO) fmt --all

.PHONY: clippy
clippy:
	$(CARGO) clippy --workspace --all-targets --all-features --locked -- -D warnings

.PHONY: lint
lint: fmt clippy

.PHONY: oracle-fetch
oracle-fetch:
	bash scripts/fetch_ft.sh

.PHONY: bench
bench: unified-oracle
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples $(BENCH_SAMPLES) --profile $(BENCH_PROFILE) --table

.PHONY: bench-quick
bench-quick: unified-oracle
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples 2 --profile $(BENCH_PROFILE) --table

.PHONY: bench-self-test
bench-self-test:
	PYTHONPYCACHEPREFIX=$(PYCACHE_DIR) $(PYTHON) -m py_compile scripts/bench_freetype.py
	$(PYTHON) scripts/bench_freetype.py --self-test

.PHONY: bench-regression
bench-regression: unified-oracle
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples $(BENCH_SAMPLES) \
		--profile $(BENCH_PROFILE) --table --require-regression-thresholds

.PHONY: record-performance-baseline
record-performance-baseline:
	$(PYTHON) scripts/bench_freetype.py --record

.PHONY: test-rust-consumer
test-rust-consumer:
	$(PYTHON) scripts/test_rust_consumer.py

.PHONY: test-c-consumer
test-c-consumer:
	$(PYTHON) scripts/test_c_consumer.py

.PHONY: c-abi-install
c-abi-install:
	$(CARGO) build --release -p fontdone-c-abi --locked
	@set -eu; \
	case "$$(uname -s)" in \
		Darwin) shared="target/release/libfontdone_c_abi.dylib" ;; \
		Linux) shared="target/release/libfontdone_c_abi.so" ;; \
		*) echo "c-abi-install currently supports Linux and macOS" >&2; exit 2 ;; \
	esac; \
	install -d "$(DESTDIR)$(PREFIX)/lib" \
		"$(DESTDIR)$(PREFIX)/lib/pkgconfig" \
		"$(DESTDIR)$(PREFIX)/include/fontdone2"; \
	install -m 0644 "$$shared" "target/release/libfontdone_c_abi.a" \
		"$(DESTDIR)$(PREFIX)/lib/"; \
	cp -R fontdone-c-abi/include/. \
		"$(DESTDIR)$(PREFIX)/include/fontdone2/"; \
	install -m 0644 fontdone-c-abi/fontdone2.pc \
		"$(DESTDIR)$(PREFIX)/lib/pkgconfig/fontdone2.pc"

.PHONY: c-abi-install-check
c-abi-install-check: test-c-consumer

.PHONY: check-c-exports
check-c-exports:
	$(PYTHON) scripts/check_c_exports.py

.PHONY: test-wasm-consumer
test-wasm-consumer:
	$(PYTHON) scripts/test_wasm_consumer.py

.PHONY: test-integrations
test-integrations: test-rust-consumer test-c-consumer check-c-exports test-wasm-consumer

.PHONY: supply-chain
supply-chain:
	$(CARGO) deny check advisories
	$(CARGO) deny check bans
	$(CARGO) deny check licenses
	$(CARGO) deny check sources
	$(CARGO) audit

.PHONY: check-versions
check-versions:
	$(PYTHON) scripts/verify_release.py --metadata-only

.PHONY: package-verify
package-verify: check-versions
	$(PYTHON) scripts/verify_release.py

.PHONY: ci-fast
ci-fast: check-generated check-font-fixtures check-docs check-versions fmt clippy doc doc-test test-fast test-rust-consumer test-c-consumer test-parity-smoke bench-self-test

.PHONY: ci-commit
ci-commit: ci-fast

.PHONY: ci
ci: ci-fast

.PHONY: ci-thorough
ci-thorough: ci-fast test-parity test-integrations c-abi-contract package-verify supply-chain test-coverage-all bench

.PHONY: release-verify
release-verify: ci-thorough c-abi-contract-complete bench-regression
	$(MAKE) check-docs

.PHONY: release-dry-run
release-dry-run: package-verify
	@echo "local ordered 3-package archive verification complete"
	@echo "after the exact root version is on crates.io, run: python3 scripts/publish_release.py --dry-run"

.PHONY: release
release: release-verify
	@if [ "$(RELEASE_APPROVED)" != "1" ]; then \
		echo "Refusing publication: set RELEASE_APPROVED=1 only after protected approval." >&2; \
		exit 2; \
	fi
	$(PYTHON) scripts/publish_release.py --publish

.PHONY: clean
clean:
	$(CARGO) clean
	@if [ -d freetype/build ]; then cmake --build freetype/build --target clean; fi
	@if [ -d tests/fixtures/outputs ]; then \
		find tests/fixtures/outputs -mindepth 1 -delete; \
	fi
	find tests/fixtures -maxdepth 1 -type f -name '*.json' -delete
