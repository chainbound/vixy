Feature: WSS (Secure WebSocket) Connection Support
  As a Vixy operator
  I want to connect to WSS endpoints
  So that I can proxy secure WebSocket connections to encrypted upstream nodes

  # To run these tests:
  #   1. Start Vixy: cargo run --release -- --config config.wss-test.toml
  #   2. Run tests: VIXY_WSS_ONLY=1 cargo test --test integration_cucumber
  #   Or use: just test-wss
  #
  # Note: These tests use public Hoodi WSS endpoints via publicnode.com (no API key required)

  Background:
    Given a public Hoodi WSS endpoint is available

  @wss @external
  Scenario: Vixy starts without TLS panics
    When Vixy is running
    Then the TLS crypto provider should be initialized
    And Vixy logs should not contain TLS panics

  @wss @external
  Scenario: WebSocket connects through Vixy to WSS upstream
    When a WebSocket client connects to Vixy at "/el/ws"
    And the client sends a JSON-RPC eth_blockNumber request
    Then the client should receive a response within 5 seconds
    And the response should be valid JSON-RPC
    And no WebSocket errors should occur

  @wss @external
  Scenario: WebSocket subscription over WSS
    When a WebSocket client connects to Vixy at "/el/ws"
    And the client subscribes to "newHeads"
    Then the client should receive a subscription ID
    And the subscription should be tracked
    And no WebSocket errors should occur
