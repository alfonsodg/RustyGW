# Remote Configuration - RustyGW

## Server
- Host: `159.203.139.159`
- SSH: `ssh root@159.203.139.159`
- App dir: `/root/Rust-API-Gateway/`

## Services
- Gateway: `./target/release/rustygw` (port 8094)
- Mock services: `python3 tests/mock_service.py <port> <name>`

## Ports
- Gateway: 8094
- Mock service 1: 9001
- Mock service 2: 9002
- Metrics: 8094/metrics
- Health: 8094/health
