#!/bin/sh
# Stop the relay before the monthly bandwidth budget is gone.
#
# MyKineID's VPS has a 200 GB/month allowance. bore has no rate limit of its
# own, so this is the hard stop: once vnstat reports 190 GB for the current
# month, the relay is stopped. It stays stopped until someone looks, because a
# surprise bill is worse than a relay that is down.
#
# A guard that cannot read usage is not a guard. Every failure path here exits
# non-zero so `systemctl --failed` shows it, instead of a green timer with a
# meter nobody is watching.
set -eu

LIMIT_GB="${FLUXCODE_RELAY_BUDGET_GB:-190}"

if ! command -v vnstat >/dev/null 2>&1; then
	echo "budget-guard: vnstat is not installed; cannot read usage" >&2
	exit 1
fi

# `--oneline b` is the only mode that reports raw bytes: `--oneline m` is not a
# valid mode (vnstat accepts only `b`), and the human modes print "1.23 GiB"
# strings that then have to be parsed back into numbers.
#
# Fields: 1 db version, 2 vnstat version, 3 interface, 4 day date, 5 day rx,
# 6 day tx, 7 day total, 8 month date, 9 month rx, 10 month tx, 11 month total.
line="$(vnstat --oneline b 2>/dev/null | head -n 1 || true)"
if [ -z "$line" ]; then
	echo "budget-guard: vnstat returned nothing" >&2
	exit 1
fi

parsed="$(printf '%s\n' "$line" | awk -F';' '{ if (NF < 11) printf "invalid"; else printf "%s", $11 }')"
if [ "$parsed" = "invalid" ]; then
	echo "budget-guard: unexpected vnstat output: $line" >&2
	exit 1
fi

if [ -z "$parsed" ]; then
	# A relay that just came up has no month entry yet; that is zero usage.
	echo "budget-guard: no usage recorded for this month yet"
	exit 0
fi

case "$parsed" in
	*[!0-9]*)
		echo "budget-guard: could not parse usage from: $line" >&2
		exit 1
		;;
esac
total_bytes="$parsed"

limit_bytes=$((LIMIT_GB * 1024 * 1024 * 1024))
echo "budget-guard: $((total_bytes / 1024 / 1024)) MiB used this month, limit $((limit_bytes / 1024 / 1024)) MiB"

if [ "$total_bytes" -ge "$limit_bytes" ]; then
	echo "budget-guard: over budget, stopping bore-server"
	systemctl stop bore-server.service
fi
