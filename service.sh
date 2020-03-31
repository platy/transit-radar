#!/bin/sh

# Transit Radar service controller
set -e

# Must be a valid filename
NAME=transit-radar
PIDFILE=/run/$NAME.pid
#This is the command to be run, give the full pathname
DAEMON=/app/transit-radar/run.sh
DAEMON_OPTS=""

case "$1" in
  start)
        echo -n "Starting daemon: "$NAME
	start-stop-daemon --start --quiet --pidfile $PIDFILE --exec $DAEMON -- $DAEMON_OPTS
        echo "."
	;;
  stop)
        echo -n "Stopping daemon: "$NAME
	start-stop-daemon --stop --quiet --oknodo --pidfile $PIDFILE
        echo "."
	;;
  restart)
        echo -n "Restarting daemon: "$NAME
	start-stop-daemon --stop --quiet --oknodo --retry 30 --pidfile $PIDFILE
	start-stop-daemon --start --quiet --pidfile $PIDFILE --exec $DAEMON -- $DAEMON_OPTS
	echo "."
	;;

  *)
	echo "Usage: "$1" {start|stop|restart}"
	exit 1
esac

exit 0