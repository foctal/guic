# Performance

GUIC treats performance as a release contract. Virtualization, bounded history,
and bounded presentation summaries must be selected before adding caches or
micro-optimizations.

## Stable-release targets

These targets are provisional until retained cross-platform render traces exist:

- pointer and keyboard state updates: p95 below 8 ms on reference hardware
- steady-state interactive rendering: p95 frame time below 16.7 ms at 60 Hz
- large collection render work: proportional to visible rows/columns plus
  bounded overscan, not total item count
- chart hover/accessibility presentation: bounded independently of dataset size
- idle surfaces: no polling loop and no unbounded background allocation
- repeated open/close and window teardown: no retained WebView, overlay,
  task, or platform-handle growth

| Surface | Reference workload | Provisional target |
| --- | --- | ---: |
| Overlay/menu/dialog | open, focus, dismiss | p95 input-to-state below 8 ms; p95 frame below 16.7 ms |
| VirtualList/DataTable/Tree | 100,000 records | viewport + overscan work only; navigation below 8 ms |
| Dock | 24 panes and 100 tabs | command application below 2 ms; no retained closed layout |
| Chart | 100,000 points in a bounded domain | hover below 8 ms; frame below 16.7 ms after render profiling |
| Editor | 100,000 lines | visible syntax pass below 4 ms; single edit below 8 ms |
| Multi-window | 8 windows repeatedly opened/closed | no retained task or native-handle growth |
| Long session | 8 hours scripted interaction | no unbounded memory or background activity growth |

Benchmarks must report hardware, OS, Rust version, optimized profile, workload,
and per-operation timing. Model benchmarks are regression signals; they do not
substitute for GPU frame traces, allocation profiles, memory measurements, or
long-session evidence.

## 2026-08-13 local model baseline

Environment: Apple M4 Pro, 24 GiB RAM, macOS 26.5.1, Rust 1.95.0, optimized
`bench` profile. These are single-run smoke measurements and are not portable
performance guarantees.

Charts (`cargo bench -p guic-charts --bench chart_models`):

| Workload | Result |
| --- | ---: |
| 100 hit-test/tick passes over a 100,000-point numeric series viewport | 37.382 ms total; 373.823 µs/pass |
| 20,000 category line/bar pointer queries over 100,000 points | 456.208 µs total; 22 ns/query |
| 120 transition snapshots over 10,000 points | 10.425 ms total; 86.876 µs/snapshot |
| 120 model passes across a 12-chart dashboard | 41.787 ms total; 348.228 µs/pass |

Category and bar pointer queries use cached derived axes and direct candidate
lookup, so their hot path performs no point-count-sized allocation or scan.
Numeric/scatter hit testing remains viewport-linear unless the application
narrows the visible domain; the first benchmark intentionally includes numeric
viewport scanning and tick generation.

Chart painting, label layout, and GPU submission are not measured by the current
model benchmark.

Editor (`cargo bench -p guic-editor --bench editor_models`):

| Workload | Result |
| --- | ---: |
| 10 searches over a 100,000-line buffer | 31.988 ms total; 3.199 ms/search |
| 2,000 insert/backspace snapshots in a 100,000-line session | 1.308 s total; 654.151 µs/operation |
| 100,000 visible-line syntax classifications | 148.132 ms total; 1.481 µs/line |

These editor model results meet the provisional model targets on the reference
machine. Pixel painting, selection geometry, IME, and scroll-follow behavior
remain outside this benchmark.
