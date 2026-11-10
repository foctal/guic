use guic_editor::{EditorBuffer, EditorPosition, EditorSelection, EditorSession};
use std::{hint::black_box, time::Instant};

fn main() {
    benchmark_large_buffer_search();
    let dense = EditorBuffer::from_text("a\u{301}".repeat(100_000));
    let started = Instant::now();
    black_box(dense.search("\u{301}"));
    report("dense_unicode_line_search", started, 1);
    benchmark_large_buffer_edits();
    benchmark_visible_syntax_pass();
    benchmark_snapshots_and_tail_ranges();
    benchmark_long_line_ranges();
    benchmark_long_line_transactions();
    benchmark_large_paste();
    benchmark_unicode_paste_and_retirement();
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

fn benchmark_snapshots_and_tail_ranges() {
    let buffer = large_buffer(100_000);
    let started = Instant::now();
    for _ in 0..100_000 {
        black_box(buffer.clone());
    }
    report("large_buffer_snapshot", started, 100_000);
    let started = Instant::now();
    for _ in 0..10_000 {
        black_box(
            buffer.text_in_range(EditorPosition::new(99_998, 0)..EditorPosition::new(99_999, 20)),
        );
    }
    report("large_buffer_tail_range", started, 10_000);
}

fn benchmark_long_line_ranges() {
    let buffer = EditorBuffer::from_text("let value = \"text🙂\"; ".repeat(50_000));
    let started = Instant::now();
    black_box(buffer.syntax_tokens(0));
    report("long_line_analysis", started, 1);
    let end = buffer.line_len(0);
    let started = Instant::now();
    for _ in 0..10_000 {
        black_box(
            buffer.text_in_range(EditorPosition::new(0, end - 20)..EditorPosition::new(0, end)),
        );
    }
    report("long_line_cached_tail_range", started, 10_000);
}

fn benchmark_long_line_transactions() {
    let buffer = EditorBuffer::from_text("let value = \"text🙂\"; ".repeat(50_000));
    black_box(buffer.syntax_tokens(0));
    let end = buffer.line_len(0);
    let mut session = EditorSession::new(buffer).history_limit(4);
    let started = Instant::now();
    for _ in 0..20 {
        session.set_selections(vec![EditorSelection::new(
            EditorPosition::new(0, end - 2),
            EditorPosition::new(0, end - 1),
        )]);
        session.insert("x");
        black_box(
            session
                .buffer()
                .text_in_range(EditorPosition::new(0, end - 20)..EditorPosition::new(0, end)),
        );
        session.undo();
        session.redo();
        session.undo();
    }
    report("long_line_replace_and_history", started, 20);
    let started = Instant::now();
    for _ in 0..50 {
        session.redo();
        black_box(session.buffer().line_len(0));
        session.undo();
    }
    report("long_line_history_and_position", started, 50);
}

fn benchmark_large_paste() {
    let paste = "let value = \"text🙂\";\r\n".repeat(20_000);
    let mut session = EditorSession::new(EditorBuffer::from_text("start\nend"));
    let started = Instant::now();
    for _ in 0..10 {
        session.insert(&paste);
        session.undo();
    }
    report("large_multiline_paste_and_undo", started, 10);
    let paste = "x".repeat(1_000_000);
    let started = Instant::now();
    for _ in 0..5 {
        session.insert(&paste);
        session.undo();
    }
    report("long_ascii_paste_and_undo", started, 5);
}

fn benchmark_unicode_paste_and_retirement() {
    let paste = "日本語e\u{301}👩‍💻🇺🇸 ".repeat(40_000);
    let mut session = EditorSession::new(EditorBuffer::from_text("prefix suffix"));
    session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(0, 7))]);
    let started = Instant::now();
    for _ in 0..3 {
        session.insert(&paste);
        black_box(session.buffer().line_len(0));
        session.undo();
    }
    report("unicode_paste_and_undo", started, 3);
    let mut session = EditorSession::new(large_buffer(100_000));
    session.insert("x");
    session.undo();
    let started = Instant::now();
    drop(session);
    report("large_history_retirement", started, 1);
}
