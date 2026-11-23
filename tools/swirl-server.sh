#!/bin/sh
ROOT=/var/www/swirl
BIN=$ROOT/target/release/swirl-server
LOG=/tmp/swirl-server.log
PIDFILE=/tmp/swirl-server.pid
PORT=$(sed -n "s/.*['\"]port['\"][[:space:]]*=>[[:space:]]*['\"]\{0,1\}\([0-9]\{1,5\}\).*/\1/p" "$ROOT/config.local.php" 2>/dev/null | head -n 1)
PORT=${SWIRL_PORT:-${PORT:-4210}}

running() {
  PID=$(pgrep -f -u fone "^$BIN --port=$PORT" | head -n 1)
  [ -n "$PID" ] && echo "$PID" > "$PIDFILE"
  [ -n "$PID" ]
}

case "$1" in
  start)
    if running; then echo "already running (pid $PID, port $PORT)"; exit 0; fi
    [ -x "$BIN" ] || { echo "missing $BIN, run: cargo build --release -p swirl-server"; exit 1; }
    cd "$ROOT" || exit 1
    if [ "$(id -un)" = "fone" ]; then
      setsid nohup "$BIN" --port="$PORT" > "$LOG" 2>&1 < /dev/null &
    else
      su fone -s /bin/sh -c "cd $ROOT && setsid nohup $BIN --port=$PORT > $LOG 2>&1 < /dev/null &"
    fi
    sleep 1
    if running; then echo "started (pid $PID, port $PORT, log $LOG)"; else echo "failed to start, see $LOG"; exit 1; fi
    ;;
  stop)
    if running; then kill "$PID"; rm -f "$PIDFILE"; echo stopped; else rm -f "$PIDFILE"; echo "not running"; fi
    ;;
  status)
    if running; then echo "running (pid $PID, port $PORT)"; curl -s "http://127.0.0.1:$PORT/status"; echo; else echo "stopped"; exit 3; fi
    ;;
  restart)
    "$0" stop; sleep 1; "$0" start
    ;;
  *)
    echo "usage: $0 start|stop|status|restart"; exit 2
    ;;
esac
