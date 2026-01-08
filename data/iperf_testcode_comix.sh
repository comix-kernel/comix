#!/bin/sh
#
# Comix-friendly iperf3 test script.
#
# Differences vs data/iperf_testcode.sh:
# - Avoids `-D` (daemon mode) which is fragile on minimal userlands.
# - Uses `-1` (one-off) and runs server in background per test case.
# - Redirects logs to /tmp to keep console clean and help debugging.
#

host="${HOST:-127.0.0.1}"
port="${PORT:-5001}"
iperf="${IPERF:-iperf3}"

# Keep consistent with the original test intent: extremely high send rate for UDP.
udp_bw="${UDP_BW:-1000G}"
duration="${DURATION:-2}"
parallel="${PARALLEL:-5}"

mkdir -p /tmp

echo "#### OS COMP TEST GROUP START iperf ####"

pass=0
fail=0

run_case() {
    name="$1"
    server_args="$2"
    client_args="$3"

    s_log="/tmp/iperf3_${name}_s.log"
    c_log="/tmp/iperf3_${name}_c.log"
    rm -rf "$s_log" "$c_log"

    echo "====== iperf $name begin ======"

    # Best-effort cleanup from previous runs.
    killall iperf3 2>/dev/null

    # Start one-off server in background.
    $iperf $server_args >"$s_log" 2>&1 &
    s_pid=$!

    # Give server time to bind/listen.
    sleep 1

    # Run client in foreground to capture return code.
    $iperf $client_args >"$c_log" 2>&1
    rc=$?

    # Best-effort: don't hang forever if server doesn't exit.
    sleep 1
    if kill -0 "$s_pid" 2>/dev/null; then
        kill "$s_pid" 2>/dev/null
        sleep 1
        kill -9 "$s_pid" 2>/dev/null
    fi
    killall iperf3 2>/dev/null

    if [ "$rc" -eq 0 ]; then
        ans="success"
        pass=$((pass + 1))
    else
        ans="fail"
        fail=$((fail + 1))
    fi

    echo "====== iperf $name end: $ans (rc=$rc) ======"
    echo "  server_log=$s_log"
    echo "  client_log=$c_log"
    echo ""
}

# Server template
srv_base="-s -4 -p $port -1"

# Client template
cli_base="-4 -c $host -p $port -t $duration -i 0"

# basic test
run_case "BASIC_UDP" "$srv_base" "$cli_base -u -b $udp_bw"
run_case "BASIC_TCP" "$srv_base" "$cli_base"

# parallel test
run_case "PARALLEL_UDP" "$srv_base" "$cli_base -u -P $parallel -b $udp_bw"
run_case "PARALLEL_TCP" "$srv_base" "$cli_base -P $parallel"

# reverse test (server sends, client receives)
run_case "REVERSE_UDP" "$srv_base" "$cli_base -u -R -b $udp_bw"
run_case "REVERSE_TCP" "$srv_base" "$cli_base -R"

echo "#### OS COMP TEST GROUP END iperf ####"
echo "iperf summary: pass=$pass fail=$fail"

# Exit non-zero if any case failed (friendlier for CI)
if [ "$fail" -ne 0 ]; then
    exit 1
fi
exit 0
