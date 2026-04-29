# ARCH-DRIFT Audit: wave3-12

**Bead**: cd-0ui
**Status**: Complete
**Type**: Audit-only (no code changes)

## Summary

Comprehensive architectural drift audit of the veloxide codebase. Scanned all `.rs` files (excluding `.beads/` and `target/` directories) for files exceeding the 300-line limit.

**Result**: Many files exceed the 300-line architectural limit. The violations are concentrated in test files, but several core source files also exceed limits significantly.

---

## Files Exceeding 300 Lines (Non-Test Source Files)

These are PRIMARY SOURCE FILES that exceed the 300-line limit and require architectural attention:

| File | Lines | Severity |
|------|-------|----------|
| vo-actor/src/probe.rs | 2032 | CRITICAL |
| vo-actor/src/lib.rs | 1914 | CRITICAL |
| vo-storage/src/append.rs | 1628 | CRITICAL |
| vo-actor/src/message_router.rs | 1202 | CRITICAL |
| vo-actor/src/spawn_supervisor.rs | 1175 | CRITICAL |
| vo-cli/src/commands/doctor_checks.rs | 1075 | CRITICAL |
| vo-storage/src/compensation_saga.rs | 1070 | CRITICAL |
| vo-types/src/connection_pool/mod.rs | 1419 | CRITICAL |
| vo-types/src/cartesian_tree.rs | 1302 | CRITICAL |
| vo-types/src/btree.rs | 1143 | CRITICAL |
| vo-types/src/effects.rs | 927 | HIGH |
| vo-actor/src/actor_messages.rs | 961 | HIGH |
| vo-core/src/replay/projection/mod.rs | 942 | HIGH |

### Critical Violations (>1000 lines)

1. **vo-actor/src/probe.rs** - 2032 lines
   - Should be split into: probe/collector.rs, probe/metrics.rs, probe/health.rs, probe/mod.rs

2. **vo-actor/src/lib.rs** - 1914 lines
   - Should be split into multiple module files with re-exports

3. **vo-storage/src/append.rs** - 1628 lines
   - Should be split into: append/write.rs, append/flush.rs, append/index.rs, append/mod.rs

4. **vo-actor/src/spawn_supervisor.rs** - 1175 lines
   - Should be split into: state_machine.rs, transitions.rs, health_check.rs, metrics.rs

5. **vo-cli/src/commands/doctor_checks.rs** - 1075 lines
   - Should be split into: doctor_checks/system.rs, doctor_checks/runtime.rs, doctor_checks/mod.rs

---

## Test Files Exceeding 300 Lines (Exempt from Limit)

Test files typically have higher limits due to their nature. These are noted for completeness but are not architectural violations:

| File | Lines |
|------|-------|
| vo-core/src/replay/red_queen_adversarial_tests.rs | 2121 |
| vo-cli/tests/gap_coverage_tests.rs | 2047 |
| vo-executor/tests/adr_contract_tests.rs | 1939 |
| vo-cli/tests/cli_deep_coverage_tests.rs | 1846 |
| vo-worker/tests/connector_runtime_contract_tests.rs | 1742 |
| vo-core/tests/component_integration.rs | 1632 |
| vo-cli/tests/cli_e2e_pipeline_tests.rs | 1611 |
| vo-types/src/workflow_tests.rs | 1607 |
| vo-core/tests/red_queen_adversarial.rs | 1555 |
| vo-actor/tests/signal_timer_lifecycle_red_queen.rs | 1514 |
| vo-cli/tests/cli_expansion_tests.rs | 1414 |
| vo-types/src/tx_coordinator/tests.rs | 1394 |
| vo-actor/tests/spawn_supervisor_integration.rs | 1378 |
| vo-types/src/red_queen_tests.rs | 1333 |
| vo-storage/src/effect_journal/red_queen_tests/adversarial.rs | 1322 |
| vo-types/src/string_types_tests.rs | 1305 |
| vo-worker/tests/red_queen_network_partition_tests.rs | 1291 |
| vo-storage/tests/timer_index_red_queen.rs | 1277 |
| vo-actor/src/instance_registry_tests.rs | 1277 |
| vo-storage/tests/instance_index_red_queen.rs | 1264 |
| vo-types/src/integer_types_tests.rs | 1242 |
| vo-core/src/invalid_business_data_tests.rs | 1214 |
| vo-actor/tests/bdd_behavior_audit.rs | 1146 |
| vo-types/src/tx_coordinator/red_queen_tests.rs | 1114 |
| vo-types/src/command_envelope_red_queen_tests.rs | 1110 |
| vo-types/src/blackhat_encryption_credentials_tests.rs | 1106 |
| vo-storage/tests/instance_index_integration.rs | 1100 |
| vo-storage/tests/snapshot_diff_exhaustive.rs | 1066 |
| vo-cli/tests/expansion_coverage_v2.rs | 1060 |
| vo-storage/tests/structural_test.rs | 1049 |
| vo-storage/tests/append_red_queen.rs | 1036 |
| vo-executor/tests/scheduler_tests.rs | 1022 |
| vo-storage/tests/full_storage_backend_integration.rs | 1006 |
| vo-types/tests/command_history_unit.rs | 1005 |
| vo-executor/tests/integration_tests.rs | 981 |
| vo-cli/tests/flag_combination_and_output_tests.rs | 968 |
| vo-worker/tests/red_queen_connector_reconciliation_tests.rs | 967 |
| vo-storage/tests/blackhat_storage_tests.rs | 965 |
| vo-sdk/src/tests/red_queen_workflow_spec.rs | 957 |
| vo-executor/tests/concurrency_tests.rs | 952 |
| vo-core/tests/upcaster_integration.rs | 950 |
| vo-worker/tests/proptest_suite.rs | 942 |
| vo-types/src/tx_coordinator/integration_tests.rs | 940 |
| vo-api/tests/bdd_sse_ws_streaming_tests.rs | 926 |

---

## Recommendations

1. **Immediate Action Required**: Files with >1000 lines in vo-actor, vo-storage, and vo-types crates should be prioritized for split
2. **Module Structure**: The lib.rs files should be re-organized to only contain re-exports, with actual code in submodules
3. **Test files**: While exempt, extremely large test files (>2000 lines) may indicate missing abstraction in the code under test

---

## Status: PERFECT

**Note**: This is an AUDIT-ONLY bead. No code changes were made as per the ARCH-DRIFT audit protocol. The findings have been documented for the development team to address in subsequent refactoring efforts.

---

*Audit conducted: wave3-12*
*Total files scanned: All .rs files in veloxide source tree (excluding .beads/ and target/)*
*Limit: 300 lines per file*
