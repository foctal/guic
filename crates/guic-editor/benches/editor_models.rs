use guic_editor::{EditorBuffer, EditorPosition, EditorSelection, EditorSession};
use std::{hint::black_box, time::Instant};

fn main() {
    benchmark_large_buffer_search();
    benchmark_large_buffer_edits();
    benchmark_visible_syntax_pass();
}

fn benchmark_large_buffer_search() {
    let buffer = large_buffer(100_000);
    let started = Instant::now();
    for _ in 0..10 {
        black_box(buffer.search("needle"));
    }
    report("large_buffer_search", started, 10);
}

fn benchmark_large_buffer_edits() {
    let mut session = EditorSession::new(large_buffer(100_000));
    let started = Instant::now();
    for iteration in 0..1_000 {
        let line = iteration % 100_000;
        session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(line, 0))]);
        session.insert("x");
        session.backspace();
    }
    report("large_buffer_edit_and_undo_snapshot", started, 2_000);
    black_box(session);
}

fn benchmark_visible_syntax_pass() {
    let buffer = large_buffer(100_000);
    let started = Instant::now();
    for frame in 0..1_000 {
        let first = frame % 99_900;
        for line in first..first + 100 {
            black_box(buffer.syntax_tokens(line));
        }
    }
    report("visible_syntax_pass", started, 100_000);
}

fn large_buffer(lines: usize) -> EditorBuffer {
    EditorBuffer::from_text(
        (0..lines)
            .map(|line| {
                if line % 1_000 == 0 {
                    format!("fn needle_{line}() {{ let value = {line}; }} // indexed")
                } else {
                    format!("let value_{line} = \"content\"; // ordinary line")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn report(name: &str, started: Instant, operations: usize) {
    let elapsed = started.elapsed();
    println!(
        "{name}: {operations} operations in {elapsed:?} ({:?}/operation)",
        elapsed / operations as u32
    );
}
