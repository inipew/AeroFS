#!/system/bin/sh
PID=$(pidof aerofs)
if [ -n "$PID" ]; then
    echo "AeroFS is currently running (PID: $PID)"
    echo "Web UI accessible at http://127.0.0.1:8080"
else
    echo "AeroFS is stopped. Checking service status..."
fi
