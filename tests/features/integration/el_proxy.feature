@integration
Feature: EL Proxy Integration Tests
  Integration tests for EL (Execution Layer) proxy functionality
  These tests run against real Ethereum nodes via Docker or Kurtosis

  Background:
    Given Vixy is running with integration config
    And the EL nodes are healthy

  @integration @el @proxy
  Scenario: Proxy forwards eth_blockNumber request
    When I send an eth_blockNumber request to Vixy
    Then I should receive a valid block number response
    And the response should be from a healthy node

  @integration @el @proxy
  Scenario: Proxy forwards eth_chainId request
    When I send an eth_chainId request to Vixy
    Then I should receive a valid chain ID response

  @integration @el @proxy
  Scenario: Proxy handles batch requests
    When I send a batch request with eth_blockNumber and eth_chainId
    Then I should receive valid responses for both methods

  @integration @el @failover
  Scenario: Proxy fails over when primary node is down
    Given the primary EL node is stopped
    When I send an eth_blockNumber request to Vixy
    Then I should receive a valid block number response
    And the response should be from the secondary node

  @integration @el @websocket
  Scenario: WebSocket proxy connects and forwards messages
    When I connect to the EL WebSocket endpoint
    And I subscribe to newHeads
    Then I should receive new block headers
