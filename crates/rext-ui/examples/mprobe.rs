fn main() {
    let line = "// Ctrl+P  command palette";
    let m = repose_text::metrics_for_textfield(line, 14.0, Some("monospace"), 400, 0, 0.0, None);
    println!("graphemes={} positions.len={}", line.chars().count(), m.positions.len());
    println!("byte_offsets.len={}", m.byte_offsets.len());
    let n = m.positions.len();
    println!("first3={:?} last3={:?}", &m.positions[..3.min(n)], &m.positions[n.saturating_sub(3)..]);
    let b = rext_core::Buffer::from_str(line);
    let row = 0;
    let ls = b.offset_of(rext_core::Point { row, column: 0 });
    let le = ls + b.line_len(row);
    println!("ls={ls} le={le} line_len={}", b.line_len(row));
    let col1 = b.point_of(le).column;
    println!("col1={col1}");
    let x = m.positions.get(col1).copied().unwrap_or(-999.0);
    println!("x_for_col({col1})={x} last={}", m.positions.last().copied().unwrap_or(-999.0));
}
