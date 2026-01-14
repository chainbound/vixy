@integration
Feature: CL Proxy Integration Tests
  Integration tests for CL (Consensus Layer) proxy functionality
  These tests run against real Beacon nodes via Docker or Kurtosis

  Background:
    Given Vixy is running with integration config
    And the CL nodes are healthy

  @integration @cl @proxy
  Scenario: Proxy forwards node health request
    When I send a GET request to /cl/eth/v1/node/health
    Then I should receive a 200 OK response

  @integration @cl @proxy
  Scenario: Proxy forwards beacon headers request
    When I send a GET request to /cl/eth/v1/beacon/headers/head
    Then I should receive a valid beacon header response
    And the response should contain a slot number

  @integration @cl @proxy
  Scenario: Proxy forwards node syncing request
    When I send a GET request to /cl/eth/v1/node/syncing
    Then I should receive a valid syncing response

  @integration @cl @failover
  Scenario: Proxy fails over when primary CL node is down
    Given the primary CL node is stopped
    When I send a GET request to /cl/eth/v1/node/health
    Then I should receive a 200 OK response
    And the response should be from the secondary CL node
