#!/bin/bash

LOGFILE=/var/log/transit-radar/log-alpha
EXECUTABLE=/app/transit-radar/transit-radar-alpha
PIDFILE=/run/transit-radar-alpha.pid

export PORT=8002 STATIC_DIR=/app/transit-radar/frontend-alpha GTFS_DIR=/app/transit-radar/gtfs LINE_COLORS=/app/transit-radar/VBB_Colours.csv RUST_BACKTRACE=1

echo "Starting $(date)" >> $LOGFILE
nohup $EXECUTABLE >> $LOGFILE 2>&1 &

echo $! > $PIDFILE
