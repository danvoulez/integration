#!/bin/bash
# =============================================================================
# LLM Gateway Modularization Script
# =============================================================================
# Usage:
#   ./modularize.sh --dry-run    # Show what will happen
#   ./modularize.sh --execute    # Actually do it
# =============================================================================

set -e

SRC="/Users/ubl-ops/Integration/llm-gateway.logline.world/src"
MAIN="$SRC/main.rs"
BACKUP="$SRC/main.rs.backup.$(date +%Y%m%d_%H%M%S)"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

MODE="${1:---dry-run}"

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_step() { echo -e "${GREEN}[STEP]${NC} $1"; }

# =============================================================================
# Module definitions: name:start:end
# =============================================================================
MODULES="
config:34:479
fuel:480:765
qc:766:816
auth:817:1306
types:1308:1539
handlers:1541:2189
streaming:2190:2890
routing:2891:3295
providers:3297:3786
utils:3787:3936
"

# =============================================================================
# Dry run: show what will be done
# =============================================================================
dry_run() {
    echo "============================================="
    echo "DRY RUN - No files will be modified"
    echo "============================================="
    echo ""
    
    log_step "1. Backup $MAIN -> $BACKUP"
    echo ""
    
    log_step "2. Extract modules:"
    total=0
    echo "$MODULES" | while IFS=: read -r mod start end; do
        [ -z "$mod" ] && continue
        lines=$((end - start + 1))
        echo "   - $mod.rs: lines $start-$end ($lines lines)"
    done
    echo ""
    
    log_step "3. Create new main.rs with:"
    echo "   - Original imports (lines 1-33)"
    echo "   - mod declarations for all modules"
    echo "   - use statements to re-export"
    echo "   - main() function (lines 3938-4312)"
    echo ""
    
    log_step "4. Estimated sizes:"
    echo "$MODULES" | while IFS=: read -r mod start end; do
        [ -z "$mod" ] && continue
        lines=$((end - start + 1))
        printf "   %-15s %4d lines\n" "$mod.rs:" "$lines"
    done
    echo "   --------------------------"
    echo "   main.rs:        ~450 lines (imports + main + cache)"
    echo ""
    
    log_warn "Run with --execute to apply changes"
}

# =============================================================================
# Execute: actually create the modules
# =============================================================================
execute() {
    echo "============================================="
    echo "EXECUTING MODULARIZATION"
    echo "============================================="
    echo ""
    
    # Step 1: Backup
    log_step "1. Creating backup..."
    cp "$MAIN" "$BACKUP"
    log_info "Backup created: $BACKUP"
    echo ""
    
    # Step 2: Extract modules
    log_step "2. Extracting modules..."
    echo "$MODULES" | while IFS=: read -r mod start end; do
        [ -z "$mod" ] && continue
        target="$SRC/$mod.rs"
        
        # Extract lines
        sed -n "${start},${end}p" "$MAIN" > "$target"
        
        lines=$(wc -l < "$target" | tr -d ' ')
        log_info "Created $mod.rs ($lines lines)"
    done
    echo ""
    
    # Step 3: Create new main.rs
    log_step "3. Creating new main.rs..."
    
    # Get original imports (lines 1-33)
    head -33 "$MAIN" > "$SRC/main_new.rs"
    
    # Add mod declarations
    cat >> "$SRC/main_new.rs" << 'MODS'

// =============================================================================
// Modules
// =============================================================================
mod config;
mod fuel;
mod qc;
mod auth;
mod types;
mod handlers;
mod streaming;
mod routing;
mod providers;
mod utils;

// Re-export commonly used items
pub use config::*;
pub use types::*;
pub use handlers::*;

MODS
    
    # Add main() and cache functions (lines 3938-4312)
    sed -n '3938,4312p' "$MAIN" >> "$SRC/main_new.rs"
    
    # Replace main.rs
    mv "$SRC/main_new.rs" "$MAIN"
    
    lines=$(wc -l < "$MAIN" | tr -d ' ')
    log_info "New main.rs created ($lines lines)"
    echo ""
    
    # Step 4: Summary
    log_step "4. Summary:"
    echo ""
    ls -la "$SRC"/*.rs
    echo ""
    
    log_warn "IMPORTANT: Run 'cargo check' to verify compilation"
    log_warn "If it fails, restore from: $BACKUP"
}

# =============================================================================
# Main
# =============================================================================
case "$MODE" in
    --dry-run)
        dry_run
        ;;
    --execute)
        execute
        ;;
    *)
        echo "Usage: $0 [--dry-run|--execute]"
        exit 1
        ;;
esac
