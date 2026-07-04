# Changelog

## [0.3.0](https://github.com/nahuel11500/ekokube/compare/v0.2.0...v0.3.0) (2026-07-04)


### Features

* **server:** drill-down endpoints — scoped /api/timeseries, per-pod /api/pods, batched /api/sparklines ([5df6dd4](https://github.com/nahuel11500/ekokube/commit/5df6dd435321daca297cff2e97abd9a7cd528332))
* **ui:** drill-down navigation and FinOps overhaul ([0de3fc1](https://github.com/nahuel11500/ekokube/commit/0de3fc1dc3dc9c81ce14cf063aa5214d287cbc03))
* **ui:** richer overview — utilization/commitment/efficiency tiles and a ([0ca400e](https://github.com/nahuel11500/ekokube/commit/0ca400e31ebd4e0fd86ecb5f29a75d01b663a105))


### Bug Fixes

* **chart:** agent rollouts freeze when any pod is unschedulable ([cf1fecc](https://github.com/nahuel11500/ekokube/commit/cf1fecc587c932e32e83a09ac7568315c99dcbc4))
* **server:** clamp average denominators to the data-covered window ([0ca400e](https://github.com/nahuel11500/ekokube/commit/0ca400e31ebd4e0fd86ecb5f29a75d01b663a105))
* **server:** count only observed collection minutes in the coverage window ([4a44602](https://github.com/nahuel11500/ekokube/commit/4a44602adcddfcc2cbadac992a6b72ff3a1d9db5))
* **server:** exact data-coverage window from raw node_usage ([3c0182c](https://github.com/nahuel11500/ekokube/commit/3c0182c1df407b006a3526f2e5f8327aaa4f3b02))
* **ui:** explicit empty state for charts instead of a degenerate axis ([e791288](https://github.com/nahuel11500/ekokube/commit/e791288a68aa80a5865701b4b57716b24dc0aefb))
