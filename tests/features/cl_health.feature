Feature: CL (Consensus Layer) Health Check
  As a Vixy operator
  I want to monitor the health of CL nodes
  So that requests are routed only to healthy nodes

  Background:
    Given a configured Vixy instance with CL nodes

  Scenario: Healthy CL node with 200 health endpoint and within lag
    Given a CL node that returns 200 on health endpoint
    And the CL node is at slot 1000
    And the CL chain head is at slot 1002
    And the max CL lag is 3 slots
    When the health check runs
    Then the CL node should be marked as healthy
    And the CL node lag should be 2 slots

  Scenario: Unhealthy CL node - health endpoint returns non-200
    Given a CL node that returns 503 on health endpoint
    And the CL node is at slot 1000
    And the CL chain head is at slot 1000
    And the max CL lag is 3 slots
    When the health check runs
    Then the CL node should be marked as unhealthy

  Scenario: Unhealthy CL node - exceeding lag threshold
    Given a CL node that returns 200 on health endpoint
    And the CL node is at slot 990
    And the CL chain head is at slot 1000
    And the max CL lag is 3 slots
    When the health check runs
    Then the CL node should be marked as unhealthy
    And the CL node lag should be 10 slots

  Scenario: Unreachable CL node
    Given a CL node that is unreachable
    When the health check runs
    Then the CL node should be marked as unhealthy

  Scenario: Update chain head from multiple CL nodes
    Given CL nodes at slots 1000, 1005, and 998
    When the health check runs
    Then the CL chain head should be 1005
