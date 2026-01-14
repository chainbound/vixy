Feature: Configuration parsing
  As a Vixy operator
  I want to configure EL and CL nodes via a TOML file
  So that I can specify which nodes to monitor and proxy

  Scenario: Parse a valid configuration file
    Given a valid TOML configuration with 2 primary EL nodes and 1 CL node
    When I parse the configuration
    Then the configuration should be loaded successfully
    And it should have 2 primary EL nodes
    And it should have 1 CL node
    And the global settings should use default values

  Scenario: Parse configuration with primary and backup EL nodes
    Given a TOML configuration with 1 primary and 2 backup EL nodes
    When I parse the configuration
    Then the configuration should be loaded successfully
    And it should have 1 primary EL node
    And it should have 2 backup EL nodes

  Scenario: Fail to parse configuration without EL nodes
    Given a TOML configuration without any EL nodes
    When I try to parse the configuration
    Then parsing should fail with an error about missing EL configuration

  Scenario: Fail to parse configuration without CL nodes
    Given a TOML configuration without any CL nodes
    When I try to parse the configuration
    Then parsing should fail with an error about missing CL configuration

  Scenario: Fail to parse configuration with invalid URL
    Given a TOML configuration with an invalid HTTP URL
    When I try to parse the configuration
    Then parsing should fail with an error about invalid URL

  Scenario: Apply default values for optional settings
    Given a minimal TOML configuration
    When I parse the configuration
    Then the configuration should be loaded successfully
    And max_el_lag_blocks should default to 5
    And max_cl_lag_slots should default to 3
    And health_check_interval_ms should default to 1000
