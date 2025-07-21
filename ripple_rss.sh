#!/bin/bash


PROGRAM_NAME="ripple"
PIDS=$(pgrep -x "$PROGRAM_NAME")

if [ -z "$PIDS" ]; then
  echo "No process found with name '$PROGRAM_NAME'"
  exit 1
fi

echo "RSS for processes named '$PROGRAM_NAME':"
echo "PID     RSS (KB)"
for PID in $PIDS; do
  if [ -r /proc/$PID/status ]; then
    RSS=$(grep VmRSS /proc/$PID/status | awk '{print $2}')
    echo "$PID     ${RSS:-0}"
  else
    echo "$PID     [permission denied]"
  fi
done
