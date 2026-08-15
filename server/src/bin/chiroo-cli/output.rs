use anyhow::Result;
use serde::Serialize;

pub(crate) fn print_table(header: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    let format_row = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if i + 1 == cells.len() {
                    cell.to_string()
                } else {
                    format!("{:<w$}  ", cell, w = widths[i])
                }
            })
            .collect()
    };

    let header: Vec<String> = header.iter().map(|h| h.to_string()).collect();
    println!("{}", format_row(&header));
    for row in rows {
        println!("{}", format_row(row));
    }
}

pub(crate) fn print_kv(fields: &[(&str, String)]) {
    let width = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, value) in fields {
        println!("{:<width$}  {value}", key, width = width);
    }
}

pub(crate) fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
