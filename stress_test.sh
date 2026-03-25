#!/bin/bash

HOST="gdev02:8094"
ENDPOINT="/metrics"

echo "🚀 Iniciando pruebas de stress para RustyGW"
echo "Target: http://$HOST$ENDPOINT"
echo

# Prueba 1: 100 requests concurrentes
echo "📊 Prueba 1: 100 requests concurrentes"
time curl -s "http://$HOST$ENDPOINT[1-100]" --parallel --parallel-max 100 > /dev/null
echo

# Prueba 2: 1000 requests secuenciales
echo "📊 Prueba 2: 1000 requests secuenciales"
start_time=$(date +%s.%N)
for i in {1..1000}; do
    curl -s "http://$HOST$ENDPOINT" > /dev/null
done
end_time=$(date +%s.%N)
duration=$(echo "$end_time - $start_time" | bc)
rps=$(echo "1000 / $duration" | bc -l)
echo "Tiempo total: ${duration}s"
echo "RPS: $(printf "%.0f" $rps)"
echo

# Prueba 3: Latencia bajo carga
echo "📊 Prueba 3: Latencia bajo carga (50 requests)"
for i in {1..50}; do
    curl -w "%{time_total}s " -s "http://$HOST$ENDPOINT" > /dev/null
done | tr ' ' '\n' | sort -n | awk '
BEGIN { sum = 0; count = 0 }
/^[0-9]/ { 
    times[count] = $1; 
    sum += $1; 
    count++ 
}
END {
    if (count > 0) {
        avg = sum / count
        min = times[0]
        max = times[count-1]
        p50 = times[int(count * 0.5)]
        p95 = times[int(count * 0.95)]
        p99 = times[int(count * 0.99)]
        
        printf "Min: %.3fs\n", min
        printf "Avg: %.3fs\n", avg  
        printf "P50: %.3fs\n", p50
        printf "P95: %.3fs\n", p95
        printf "P99: %.3fs\n", p99
        printf "Max: %.3fs\n", max
    }
}'
