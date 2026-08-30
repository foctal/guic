use std::{hint::black_box, time::Instant};

use guic_terminal::TerminalModel;

fn main() {
    benchmark_dense_output();
    benchmark_scrollback_navigation();
    benchmark_resize_reflow();
    benchmark_many_panes();
    benchmark_long_session();
    benchmark_mixed_control_stream();
}

fn benchmark_long_session() {
    let mut terminal = TerminalModel::new(120, 40).max_scrollback(50_000);
    let started = Instant::now();
    for batch in 0..1_000 {
        for line in 0..100 {
            terminal.write(&format!(
                "batch={batch:04} line={line:03} status=ok payload={}\r\n",
                "x".repeat(32)
            ));
        }
        if batch % 25 == 0 {
            terminal.resize(80, 30);
            terminal.resize(120, 40);
        }
    }
    report("long_session_100k_lines", started, 100_000);
    println!(
        "long_session_estimated_heap_bytes: {}",
        terminal.estimated_heap_bytes()
    );
    black_box(terminal);
}

fn benchmark_mixed_control_stream() {
    let mut terminal = TerminalModel::new(160, 48).max_scrollback(10_000);
    let payload = (0..20_000)
        .map(|index| {
            format!(
                "\x1b[{};{}H\x1b[38;2;{};{};{}mvalue-{index}\x1b[0m\x1b]8;;https://example.invalid/{index}\x1b\\link\x1b]8;;\x1b\\\r\n",
                index % 48 + 1,
                index % 160 + 1,
                index % 255,
                index * 3 % 255,
                index * 7 % 255
            )
        })
        .collect::<String>();
    let started = Instant::now();
    terminal.write(&payload);
    report("mixed_csi_osc_stream", started, 20_000);
    black_box(terminal);
}

fn benchmark_dense_output() {
    let mut terminal = TerminalModel::new(160, 48).max_scrollback(20_000);
    let payload = (0..10_000)
        .map(|index| {
            format!(
                "\\x1b[38;5;{}m{:05} INFO request completed duration={}ms\\x1b[0m\\r\\n",
                index % 256,
                index,
                index % 200
            )
        })
        .collect::<String>();
    let started = Instant::now();
    terminal.write(black_box(&payload));
    report("dense_sgr_output", started, 10_000);
    black_box(terminal);
}

fn benchmark_scrollback_navigation() {
    let mut terminal = populated_terminal(120, 40, 20_000);
    let started = Instant::now();
    for _ in 0..1_000 {
        terminal.scroll_up(3);
        black_box(terminal.viewport_lines());
        terminal.scroll_down(2);
    }
    report("scrollback_viewport", started, 1_000);
}

fn benchmark_resize_reflow() {
    let mut terminal = populated_terminal(160, 50, 5_000);
    let started = Instant::now();
    for iteration in 0..200 {
        if iteration % 2 == 0 {
            terminal.resize(80, 40);
        } else {
            terminal.resize(160, 50);
        }
    }
    report("wrapped_line_resize", started, 200);
    black_box(terminal);
}

fn benchmark_many_panes() {
    let mut panes = (0..24)
        .map(|_| TerminalModel::new(100, 30).max_scrollback(2_000))
        .collect::<Vec<_>>();
    let payload = "status=ok message=parallel-pane-update\\r\\n";
    let started = Instant::now();
    for _ in 0..500 {
        for pane in &mut panes {
            pane.write(payload);
        }
    }
    report("twenty_four_panes", started, 12_000);
    black_box(panes);
}

fn populated_terminal(columns: usize, rows: usize, lines: usize) -> TerminalModel {
    let mut terminal = TerminalModel::new(columns, rows).max_scrollback(lines);
    for index in 0..lines {
        terminal.write(&format!(
            "{index:05} The quick brown fox jumps over the lazy dog and reflows safely.\\r\\n"
        ));
    }
    terminal
}

fn report(name: &str, started: Instant, iterations: usize) {
    let elapsed = started.elapsed();
    println!(
        "{name}: {iterations} operations in {elapsed:?} ({:?}/operation)",
        elapsed / iterations as u32
    );
}
