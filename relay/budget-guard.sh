#!/bin/sh
# Stop the relay before the monthly bandwidth budget is gone.
#
# MyKineID's VPS has a 200 GB/month allowance. bore has no rate limit of its
# own, so this is the hard stop: once vnstat reports 190 GB for the current
# month, the relay is stopped. It stays stopped until someone looks, because a
# surprise bill is worse than a relay that is down.
set -eu

LIMIT_GB="${FLUXCODE_RELAY_BUDGET_GB:-190}"

if ! command -v vnstat >/dev/null 2>&1; then
    echo "budget-guard: vnstat is not installed; cannot read usage" >&2
    exit 0
fi

# live;<iface>;<month>;<rx>;<tx>;<total>;...
# Values carry a unit ("1.23 GiB"), which awk below turns into whole MiB.
line="$(vnstat --oneline m 2>/dev/null | head -n 1 || true)"
[ -n "$line" ] || { echo "budget-guard: vnstat returned nothing" >&2; exit 0; }

total_mib="$(printf '%s\n' "$line" | awk -F';' '
    {
        n = split($6, a, " ")
        v = a[1] + 0
        u = (n >= 2) ? a[2] : "MiB"
        if (u == "TiB")       m = 1024 * 1024
        else if (u == "GiB")  m = 1024
        else if (u == "MiB")  m = 1
        else if (u == "KiB")  m = 1 / 1024
        else if (u == "B")    m = 1 / 1024 / 1024
        else                  m = 1
        printf "%d", v * m
    }')"
[ -n "$total_mib" ] || { echo "budget-guard: could not parse usage" >&2; exit 0; }

limit_mib=$((LIMIT_GB * 1024))
echo "budget-guard: ${total_mib} MiB used this month, limit ${limit_mib} MiB"

if [ "$total_mib" -ge "$limit_mib" ]; then
    echo "budget-guard: over budget, stopping bore-server"
    systemctl stop bore-server.service
fi
