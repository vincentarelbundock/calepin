#!/bin/bash
set -e

N=10000
P=50
SECTIONS=5
SUBSECTIONS=15
CALEPIN="calepin"

parse_time() {
    local real
    real=$(echo "$1" | grep real | awk '{print $2}')
    local minutes seconds
    minutes=$(echo "$real" | sed 's/m.*//')
    seconds=$(echo "$real" | sed 's/.*m//' | sed 's/s//')
    echo "$minutes * 60 + $seconds" | bc
}

run_bench() {
    local tag="$1"
    local extra_flags="$2"

    rm -f *.html
    local time_out
    time_out=$( { time "$CALEPIN" render *.qmd -q $extra_flags 2>&1 ; } 2>&1 )
    local total
    total=$(printf "%.3f" "$(parse_time "$time_out")")
    local ms
    ms=$(printf "%.2f" "$(echo "scale=4; $total * 1000 / $N" | bc)")

    eval "total_${tag}=$total"
    eval "ms_${tag}=$ms"
}

echo "$N files, $P paragraphs each, $SECTIONS sections x $(( SUBSECTIONS / SECTIONS )) subsections per file"

# --- Complexity 0 ---
cd /tmp; rm -rf gibberish
"$CALEPIN" new gibberish -n "$N" -p "$P" -c 0 2>/dev/null
cd gibberish

run_bench "c0_no" "--no-highlight"
run_bench "c0_hl" ""
c0_ratio=$(printf "%.1f" "$(echo "scale=2; $total_c0_hl / $total_c0_no" | bc)")

echo ""
echo "  Complexity 0 -- prose only:"
echo "    Without highlighting: ${total_c0_no}s total, ${ms_c0_no}ms/file"
echo "    With highlighting:    ${total_c0_hl}s total, ${ms_c0_hl}ms/file"
echo "    Ratio: ${c0_ratio}x"

# --- Complexity 1 ---
cd /tmp; rm -rf gibberish
"$CALEPIN" new gibberish -n "$N" -p "$P" -c 1 2>/dev/null
cd gibberish

run_bench "c1_no" "--no-highlight"
run_bench "c1_hl" ""
c1_ratio=$(printf "%.1f" "$(echo "scale=2; $total_c1_hl / $total_c1_no" | bc)")

echo ""
echo "  Complexity 1 -- prose + code chunks:"
echo "    Without highlighting: ${total_c1_no}s total, ${ms_c1_no}ms/file"
echo "    With highlighting:    ${total_c1_hl}s total, ${ms_c1_hl}ms/file"
echo "    Ratio: ${c1_ratio}x"

# --- Complexity 2 ---
cd /tmp; rm -rf gibberish
"$CALEPIN" new gibberish -n "$N" -p "$P" -c 2 2>/dev/null
cd gibberish

run_bench "c2_no" "--no-highlight"
run_bench "c2_hl" ""
c2_ratio=$(printf "%.1f" "$(echo "scale=2; $total_c2_hl / $total_c2_no" | bc)")

echo ""
echo "  Complexity 2 -- prose + code + cross-refs/footnotes/citations/tables:"
echo "    Without highlighting: ${total_c2_no}s total, ${ms_c2_no}ms/file"
echo "    With highlighting:    ${total_c2_hl}s total, ${ms_c2_hl}ms/file"
echo "    Ratio: ${c2_ratio}x"

echo ""
