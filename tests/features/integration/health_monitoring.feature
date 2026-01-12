@integration
Feature: Health Monitoring Integration Tests
  Integration tests for health monitoring functionality
  These tests verify that Vixy correctly monitors real node health

  Background:
    Given Vixy is running with integration config

  @integration @health @status
  Scenario: Status endpoint returns all node states
    When I request the status endpoint
    Then I should receive a JSON response
    And the response should contain EL node statuses
    And the response should contain CL node statuses
    And all nodes should show as healthy

  @integration @health @monitoring
  Scenario: Health monitor detects node going down
    Given all nodes are healthy
    When the primary EL node is stopped
    And I wait for the health check interval
    And I request the status endpoint
    Then the primary EL node should show as unhealthy

  @integration @health @monitoring
  Scenario: Health monitor detects node recovering
    Given the primary EL node was stopped
    When the primary EL node is restarted
    And I wait for the health check interval
    And I request the status endpoint
    Then the primary EL node should show as healthy

  @integration @health @lag
  Scenario: Health monitor calculates correct lag
    When I request the status endpoint
    Then each EL node should have a lag value
    And each CL node should have a lag value
    And healthy nodes should have lag within threshold

  @integration @metrics
  Scenario: Prometheus metrics are exposed
    When I request the metrics endpoint
    Then I should receive Prometheus format metrics
    And the metrics should include node health gauges
