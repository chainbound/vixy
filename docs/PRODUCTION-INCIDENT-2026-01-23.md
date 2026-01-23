# Production Incident - 2026-01-23
## WebSocket Reconnection Issues Under Load

### Incident Summary
- **Date**: January 23, 2026
- **Duration**: ~3 hours
- **Impact**: WebSocket clients experiencing "context deadline exceeded" errors after node reconnection
- **Root Cause**: Subscription replay responses forwarded to clients, breaking JSON-RPC state

---

## Documentation

### 1. [WEBSOCKET-RECONNECTION-FIX.md](../WEBSOCKET-RECONNECTION-FIX.md)
**Complete fix plan with implementation details**

Contains:
- Root cause analysis (5 issues identified)
- Design principles violated
- Fix plan in 3 phases (P0, P1, P2)
- Complete code implementations (TDD)
- Rollout plan and success metrics

**Start here for implementation.**

---

### 2. [TESTING-IMPROVEMENTS.md](../TESTING-IMPROVEMENTS.md)
**Why tests didn't catch this + improved test plan**

Contains:
- Gap analysis (8 critical gaps)
- Why each issue was missed
- Improved test plan in 4 phases
- Complete test implementations
- Property-based and chaos testing

**Start here for test improvements.**

---

## Quick Reference

### Confirmed Issues
1. ✅ **Issue #2** (ROOT CAUSE): Subscription replay responses break clients
2. ✅ **Issue #5**: Never switches back to primary
3. ❓ **Issue #1**: Messages lost during reconnection (likely, not confirmed)
4. ❓ **Issue #3**: Health checks block without timeout (possible)

### Implementation Priority
- **Phase 0 (24h)**: Fix Issues #2 and #5 - CRITICAL HOTFIX
- **Phase 1 (1wk)**: Fix Issues #1 and #3 - Prevent message loss
- **Phase 2 (2wk)**: Optimization - Concurrent health checks

### Test Priority
- **Phase 1 (1wk)**: Add 4 critical integration tests
- **Phase 2 (2wk)**: Property-based testing
- **Phase 3 (2wk)**: Chaos testing framework
- **Phase 4 (4wk)**: Load/soak testing

---

## Related Documents
- [AGENT.md](../AGENT.md) - Original development guide
- [DIARY.md](../DIARY.md) - Development log
- [INTEGRATION_TESTS.md](../INTEGRATION_TESTS.md) - Current integration test docs

---

## Timeline

```
T-3h:  Nodes unhealthy under load
T-2h:  4 reconnections occurred
       ↓ Subscription replay responses forwarded to clients
       ↓ Clients' JSON-RPC state breaks
       ↓ All future requests timeout
T-1h:  Half the clients give up, disconnect, reconnect fresh
       ↓ New connections work
       ↓ Old connections still broken
T=0:   Investigation begins
       ↓ Metrics show only backup connected
       ↓ Primary recovered 3h ago, never switched back
       ↓ Active subscriptions dropped from 4-5 to 1
```

---

## Immediate Actions

1. **Restart Vixy** to clear broken connections
2. **Implement Phase 0 fixes** (Issues #2 and #5)
3. **Add Phase 1 critical tests** before deploying
4. **Deploy with canary** (10% → 25% → 50% → 100%)

---

## Success Metrics

### Before Fix
- Request failure rate: ~5% after reconnection
- Subscriptions broken: ~40%
- Time stuck on backup: Indefinite
- Connections become "zombies"

### After Phase 0 (Target)
- Request failure rate: <0.01%
- Subscriptions broken: <0.1%
- Auto-switch to primary: <2 seconds
- Zero zombie connections

---

## Lessons Learned

1. **Design for Failure**: Proxies must be transparent during backend failures
2. **Test Edge Cases**: Happy path tests aren't enough
3. **Load Testing Matters**: Concurrency bugs only appear under load
4. **Monitor Everything**: Good metrics would have caught Issue #5 immediately
5. **Chaos Engineering**: Inject failures regularly, don't wait for production

---

## Next Steps

See [WEBSOCKET-RECONNECTION-FIX.md](../WEBSOCKET-RECONNECTION-FIX.md) for detailed implementation plan.
