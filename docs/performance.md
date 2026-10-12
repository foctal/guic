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
