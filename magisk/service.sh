#!/system/bin/sh
MODDIR="${0%/*}"
DATA_DIR="/data/adb/aerofs"
STORAGE_DIR="$DATA_DIR/storage"
TEMP_DIR="$DATA_DIR/temp"
LOG_DIR="$DATA_DIR/logs"
LOG_FILE="$LOG_DIR/aerofs.log"
BIN="$MODDIR/bin/aerofs"

# Wait for boot completion before starting service
until [ "$(getprop sys.boot_completed)" = "1" ]; do
    sleep 3
done

# Ensure all runtime mutable directories exist
mkdir -p "$DATA_DIR" "$STORAGE_DIR" "$TEMP_DIR" "$LOG_DIR"
chmod 700 "$DATA_DIR" "$STORAGE_DIR" "$TEMP_DIR" "$LOG_DIR"

# Simple log rotation: keep log below 5 MB
if [ -f "$LOG_FILE" ] && [ "$(stat -c%s "$LOG_FILE" 2>/dev/null || echo 0)" -gt 5242880 ]; then
    mv -f "$LOG_FILE.1" "$LOG_FILE.2" 2>/dev/null
    mv -f "$LOG_FILE" "$LOG_FILE.1" 2>/dev/null
fi

# Fallback: if binary is in module directory or system path
if [ ! -f "$BIN" ]; then
    BIN="/data/adb/modules/aerofs/bin/aerofs"
fi

if [ ! -x "$BIN" ]; then
    chmod 755 "$BIN" 2>/dev/null
fi

# Export deterministic environment configuration for Android
export AEROFS_ENV="production"
export AEROFS_HOST="127.0.0.1"
export AEROFS_PORT="8080"
export AEROFS_DEFAULT_LOCAL_ROOT="$STORAGE_DIR"
export AEROFS_DATABASE_URL="sqlite://$DATA_DIR/filemanager.db?mode=rwc"
export AEROFS_TEMP_DIR="$TEMP_DIR"
export AEROFS_MAX_TRANSFERS="2"
export TMPDIR="$TEMP_DIR"

# Watchdog supervisor loop with exponential backoff
BACKOFF=3
while true; do
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting AeroFS daemon..." >> "$LOG_FILE"
    
    # Run AeroFS daemon redirecting logs
    "$BIN" >> "$LOG_FILE" 2>&1
    EXIT_CODE=$?
    
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] AeroFS exited with code $EXIT_CODE. Restarting in ${BACKOFF}s..." >> "$LOG_FILE"
    sleep $BACKOFF
    
    # Exponential backoff up to 60 seconds
    if [ $BACKOFF -lt 60 ]; then
        BACKOFF=$((BACKOFF * 2))
    fi
done
