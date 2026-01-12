Feature: EL (Execution Layer) Health Check
  As a Vixy operator
  I want to monitor the health of EL nodes
  So that requests are routed only to healthy nodes

  Background:
    Given a configured Vixy instance with EL nodes

  Scenario: Healthy EL node within lag threshold
    Given an EL node at block 1000
    And the EL chain head is at block 1002
    And the max EL lag is 5 blocks
    When the health check runs
    Then the EL node should be marked as healthy
    And the EL node lag should be 2 blocks

  Scenario: Unhealthy EL node exceeding lag threshold
    Given an EL node at block 990
    And the EL chain head is at block 1000
    And the max EL lag is 5 blocks
    When the health check runs
    Then the EL node should be marked as unhealthy
    And the EL node lag should be 10 blocks

  Scenario: Unreachable EL node
    Given an EL node that is unreachable
    When the health check runs
    Then the EL node should be marked as unhealthy

  Scenario: EL node returns invalid response
    Given an EL node that returns an invalid response
    When the health check runs
    Then the EL node should be marked as unhealthy

  Scenario: Update chain head from multiple nodes
    Given EL nodes at blocks 1000, 1005, and 998
    When the health check runs
    Then the EL chain head should be 1005
