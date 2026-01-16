# Vixy Grafana Dashboard

This directory contains a pre-configured Grafana dashboard for monitoring Vixy, the Ethereum node proxy.

## Features

The dashboard provides comprehensive monitoring across three main areas:

### Overview
- Total healthy EL and CL nodes
- EL failover status
- Active WebSocket connections

### Execution Layer (EL) Monitoring
- **Block Height**: Chain head and individual node block numbers
- **Node Lag**: Block lag for each node by tier (primary/backup)
- **Health Status**: Real-time health status for all EL nodes
- **Request Rate**: Requests per second to each node
- **Request Latency**: P50, P95, P99 latency percentiles
- **Failover Status**: Total failovers and current failover state

### Consensus Layer (CL) Monitoring
- **Slot Height**: Chain head and individual node slot numbers
- **Node Lag**: Slot lag for each node
- **Health Status**: Real-time health status for all CL nodes
- **Request Rate**: Requests per second to each node
- **Request Latency**: P50, P95, P99 latency percentiles

### WebSocket Monitoring
- **Active Connections & Subscriptions**: Real-time count of active connections and subscriptions
- **Connection & Subscription Rates**: New connections and subscriptions per second
- **Message Rate**: Upstream and downstream message throughput
- **Reconnections**: Successful reconnections and failed attempts
- **Upstream Node Status**: Which EL node is currently connected for WebSocket proxy

## Installation

### 1. Import the Dashboard

1. Open Grafana UI
2. Navigate to **Dashboards** → **Import**
3. Upload `vixy-dashboard.json` or paste its contents
4. Select your Prometheus datasource
5. Click **Import**

### 2. Configure Prometheus

Ensure Prometheus is scraping Vixy's metrics endpoint. Add this to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'vixy'
    static_configs:
      - targets: ['localhost:9090']  # Adjust host:port as needed
```

Vixy exposes metrics on the `/metrics` endpoint (default port 9090, configurable in `config.toml`).

### 3. Verify Metrics

Check that Vixy is exposing metrics:

```bash
curl http://localhost:9090/metrics
```

You should see metrics prefixed with `vixy_`.

## Metrics Reference

All metrics are prefixed with `vixy_`:

### EL Metrics
- `vixy_el_requests_total` - Counter: Total EL requests (labels: node, tier)
- `vixy_el_request_duration_seconds` - Histogram: EL request latency (labels: node, tier)
- `vixy_el_node_block_number` - Gauge: Current block number (labels: node, tier)
- `vixy_el_node_lag_blocks` - Gauge: Block lag (labels: node, tier)
- `vixy_el_node_healthy` - Gauge: Health status 0/1 (labels: node, tier)
- `vixy_el_failover_active` - Gauge: Failover active 0/1
- `vixy_el_failovers_total` - Counter: Total failovers triggered
- `vixy_el_chain_head` - Gauge: Current chain head block
- `vixy_el_healthy_nodes` - Gauge: Count of healthy nodes

### CL Metrics
- `vixy_cl_requests_total` - Counter: Total CL requests (labels: node)
- `vixy_cl_request_duration_seconds` - Histogram: CL request latency (labels: node)
- `vixy_cl_node_slot` - Gauge: Current slot (labels: node)
- `vixy_cl_node_lag_slots` - Gauge: Slot lag (labels: node)
- `vixy_cl_node_healthy` - Gauge: Health status 0/1 (labels: node)
- `vixy_cl_chain_head` - Gauge: Current chain head slot
- `vixy_cl_healthy_nodes` - Gauge: Count of healthy nodes

### WebSocket Metrics
- `vixy_ws_connections_active` - Gauge: Active WebSocket connections
- `vixy_ws_connections_total` - Counter: Total connections established (lifetime)
- `vixy_ws_messages_total` - Counter: Total messages (labels: direction)
- `vixy_ws_reconnections_total` - Counter: Successful reconnections
- `vixy_ws_reconnection_attempts_total` - Counter: Reconnection attempts (labels: status)
- `vixy_ws_subscriptions_active` - Gauge: Active subscriptions
- `vixy_ws_subscriptions_total` - Counter: Total subscriptions created (lifetime)
- `vixy_ws_upstream_node` - Gauge: Current upstream node 0/1 (labels: node)

## Dashboard Customization

### Adjusting Thresholds

Edit panel thresholds to match your requirements:

1. Click a panel title → **Edit**
2. Go to **Field** tab
3. Modify **Thresholds** values
4. Save

### Adding Alerts

To add alerts to any panel:

1. Click panel title → **Edit**
2. Go to **Alert** tab
3. Create alert rule with desired conditions
4. Configure notification channels
5. Save

## Refresh Rate

The dashboard auto-refreshes every 10 seconds by default. Adjust this in the dashboard settings (top-right corner).

## Troubleshooting

### No Data Showing

1. Verify Vixy is running and metrics endpoint is accessible
2. Check Prometheus is scraping Vixy (Prometheus UI → Status → Targets)
3. Verify datasource is configured correctly in Grafana
4. Check time range in dashboard (top-right)

### Incomplete Metrics

Some metrics only appear when:
- EL nodes are configured and running
- CL nodes are configured and running
- WebSocket connections are active
- Failover has been triggered (for failover metrics)

## License

This dashboard is part of the Vixy project and is licensed under the MIT License.
