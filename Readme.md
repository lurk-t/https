wrk -c 1 -d 10 -t 1 -H 'Connection: close' --latency https://127.0.0.1:8080/0Kb;

cargo run --release -j 16 -- -c certs/RSA_2048.crt -k certs/RSA_2048.key -a 0.0.0.0:8080
