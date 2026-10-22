//! Text tables in the style of Python's `tabulate(..., tablefmt='grid')`

/// Numbers are aligned on the decimal point (right-aligned), text to the left
fn is_number(cell: &str) -> bool {
    let s = cell.trim();
    !s.is_empty() && s.parse::<f64>().is_ok()
}

fn width(s: &str) -> usize {
    s.chars().count()
}

/// Pad the numbers of a column so that their decimal points line up
fn align_decimals(cells: &[String]) -> Vec<String> {
    let split = |c: &str| -> (String, String) {
        let c = c.trim();
        match c.find('.') {
            Some(i) if !c.contains(['e', 'E']) => (c[..i].to_string(), c[i..].to_string()),
            _ => (c.to_string(), String::new()),
        }
    };
    let parts: Vec<(String, String)> = cells.iter().map(|c| split(c)).collect();
    let max_frac = parts.iter().map(|(_, f)| width(f)).max().unwrap_or(0);
    parts.into_iter().map(|(int, frac)| format!("{int}{frac}{}", " ".repeat(max_frac - width(&frac)))).collect()
}

pub fn grid(headers: &[&str], rows: &[Vec<String>]) -> String {
    let columns = headers.len();
    let mut cells: Vec<Vec<String>> =
        (0..columns).map(|c| rows.iter().map(|r| r.get(c).cloned().unwrap_or_default()).collect()).collect();
    let numeric: Vec<bool> = cells.iter().map(|col| !col.is_empty() && col.iter().all(|c| is_number(c))).collect();
    for (col, &num) in cells.iter_mut().zip(&numeric) {
        if num {
            *col = align_decimals(col);
        }
    }
    let widths: Vec<usize> =
        (0..columns).map(|c| cells[c].iter().map(|s| width(s)).max().unwrap_or(0).max(width(headers[c]) + 2)).collect();

    let line = |fill: char| -> String {
        let segments: Vec<String> = widths.iter().map(|w| fill.to_string().repeat(w + 2)).collect();
        format!("+{}+", segments.join("+"))
    };
    let row = |values: &[&str]| -> String {
        let segments: Vec<String> = values
            .iter()
            .enumerate()
            .map(|(c, v)| {
                let pad = " ".repeat(widths[c] - width(v));
                if numeric[c] {
                    format!(" {pad}{v} ")
                } else {
                    format!(" {v}{pad} ")
                }
            })
            .collect();
        format!("|{}|", segments.join("|"))
    };

    let mut out = vec![line('-'), row(headers), line('=')];
    for r in 0..rows.len() {
        let values: Vec<&str> = (0..columns).map(|c| cells[c][r].as_str()).collect();
        out.push(row(&values));
        out.push(line('-'));
    }
    if rows.is_empty() {
        out.push(line('-'));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(data: &[&[&str]]) -> Vec<Vec<String>> {
        data.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
    }

    // Expected outputs produced by tabulate 0.9 (tablefmt='grid')
    #[test]
    fn text_and_numbers() {
        let table = grid(&["H", "Num", "Dec", "E"], &rows(&[&["a", "12", "1.5", "x"], &["bb", "3", "10.25", ""]]));
        assert_eq!(
            table,
            "+-----+-------+-------+-----+\n\
             | H   |   Num |   Dec | E   |\n\
             +=====+=======+=======+=====+\n\
             | a   |    12 |  1.5  | x   |\n\
             +-----+-------+-------+-----+\n\
             | bb  |     3 | 10.25 |     |\n\
             +-----+-------+-------+-----+"
        );
    }

    #[test]
    fn no_rows() {
        assert_eq!(
            grid(&["Interface", "MAC"], &[]),
            "+-------------+-------+\n\
             | Interface   | MAC   |\n\
             +=============+=======+\n\
             +-------------+-------+"
        );
    }

    #[test]
    fn mixed_column_is_text() {
        assert_eq!(
            grid(&["H", "Mixed"], &rows(&[&["a", "12"], &["bb", "x"]])),
            "+-----+---------+\n\
             | H   | Mixed   |\n\
             +=====+=========+\n\
             | a   | 12      |\n\
             +-----+---------+\n\
             | bb  | x       |\n\
             +-----+---------+"
        );
    }
}
