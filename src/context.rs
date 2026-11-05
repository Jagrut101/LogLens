use crate::analysis::endian_is_little;
use anyhow::{Context, Result};
use calamine::{open_workbook_auto, Reader};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TableData {
    pub filename: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl TableData {
    pub fn empty() -> Self { Self::default() }
    pub fn is_empty(&self) -> bool { self.rows.is_empty() }
    pub fn col(&self, row: usize, names: &[&str]) -> String {
        let Some(r) = self.rows.get(row) else { return String::new(); };
        for name in names {
            if let Some(i) = self.headers.iter().position(|h| h.eq_ignore_ascii_case(name)) {
                return r.get(i).cloned().unwrap_or_default();
            }
        }
        String::new()
    }
    pub fn unique(&self, names: &[&str]) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0..self.rows.len() {
            let v = self.col(i, names);
            if !v.trim().is_empty() && !out.contains(&v) { out.push(v); }
        }
        out.sort(); out
    }
}

pub fn read_table(path: &Path) -> Result<TableData> {
    read_table_for(path, None)
}

pub fn read_table_for(path: &Path, sheet_hint: Option<&str>) -> Result<TableData> {
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "csv" => read_csv(path),
        "xlsx" | "xls" => read_excel(path, sheet_hint),
        _ => anyhow::bail!("Unsupported context file: {}", path.display()),
    }
}

fn read_csv(path: &Path) -> Result<TableData> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_path(path)?;
    let headers = rdr.headers()?.iter().map(|s| s.trim().to_string()).collect::<Vec<_>>();
    let mut rows = Vec::new();
    for rec in rdr.records() { rows.push(rec?.iter().map(|s| s.trim().to_string()).collect()); }
    Ok(TableData { filename: file_name(path), headers, rows })
}

fn read_excel(path: &Path, sheet_hint: Option<&str>) -> Result<TableData> {
    let mut wb = open_workbook_auto(path).with_context(|| format!("open {}", path.display()))?;
    let names = wb.sheet_names();
    let sheet = sheet_hint
        .and_then(|hint| names.iter().find(|name| name.eq_ignore_ascii_case(hint)).cloned())
        .or_else(|| names.first().cloned())
        .context("Workbook has no sheets")?;
    let range = wb.worksheet_range(&sheet)?;
    let mut iter = range.rows();
    let headers = iter.next().unwrap_or(&[]).iter().map(|v| v.to_string().trim().to_string()).collect::<Vec<_>>();
    let rows = iter.map(|r| r.iter().map(|v| v.to_string().trim().to_string()).collect()).collect();
    Ok(TableData { filename: file_name(path), headers, rows })
}

fn file_name(path: &Path) -> String { path.file_name().and_then(|x| x.to_str()).unwrap_or("context").to_string() }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SignalDefinition {
    #[serde(default)]
    pub protocol: String,
    pub name: String,
    pub channel: String,
    pub frame_id: String,
    pub start_byte: usize,
    pub start_bit: usize,
    pub bit_length: usize,
    pub endian: String,
    pub signed: bool,
    #[serde(default)]
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
    pub unit: String,
}

pub fn signal_definitions(table: &TableData) -> Vec<SignalDefinition> {
    let mut out = Vec::new();
    for i in 0..table.rows.len() {
        let name = table.col(i, &["signal_name", "SignalName", "Signal", "Name"]);
        let frame_id = table.col(i, &["message_id", "frame_id", "FrameID", "CAN_ID", "CAN ID", "ID"]);
        if name.is_empty() || frame_id.is_empty() { continue; }
        let start_byte_text = table.col(i, &["byte_position","BytePosition","byte_pos","start_byte", "StartByte", "Byte", "ByteIndex"]);
        let start_bit_text = table.col(i, &["bit_position","BitPosition","bit_pos","start_bit", "StartBit", "Bit"]);
        let endian = { let e = table.col(i, &["endian", "Endian", "ByteOrder"]); if e.is_empty() { "Intel / Little Endian".into() } else { e } };
        let default_start_bit = if endian_is_little(&endian) { 0 } else { 7 };
        let raw_start_bit = parse_usize(&start_bit_text, default_start_bit);
        let (start_byte, start_bit) = if start_byte_text.trim().is_empty() {
            (raw_start_bit / 8, raw_start_bit % 8)
        } else {
            (parse_usize(&start_byte_text, 0), raw_start_bit)
        };
        out.push(SignalDefinition {
            protocol: table.col(i, &["protocol", "Protocol", "bus", "Bus", "bus_type", "BusType"]),
            name,
            channel: table.col(i, &["channel_id", "Channel", "ChannelID", "Channel ID"]),
            frame_id,
            start_byte,
            start_bit,
            bit_length: parse_usize(&table.col(i, &["length", "bit_length", "BitLength", "Length", "SignalLength"]), 8).max(1),
            endian,
            signed: matches!(table.col(i, &["signed", "Signed", "IsSigned"]).to_ascii_lowercase().as_str(), "true"|"1"|"yes"|"signed"),
            data_type: {
                let dt=table.col(i, &["data_type","DataType","Data Type","type","Type"]);
                if dt.trim().is_empty() {
                    if matches!(table.col(i, &["signed", "Signed", "IsSigned"]).to_ascii_lowercase().as_str(), "true"|"1"|"yes"|"signed") { "Signed Integer".into() } else { "Unsigned Integer".into() }
                } else { dt }
            },
            scale: parse_f64(&table.col(i, &["factor", "scale", "Scale", "Factor"]), 1.0),
            offset: parse_f64(&table.col(i, &["offset", "Offset"]), 0.0),
            unit: table.col(i, &["unit", "Unit"]),
        });
    }
    out
}

pub fn rows_as_maps(table: &TableData) -> Vec<BTreeMap<String,String>> {
    table.rows.iter().map(|r| table.headers.iter().enumerate().map(|(i,h)|(h.clone(),r.get(i).cloned().unwrap_or_default())).collect()).collect()
}

fn parse_usize(s:&str, default:usize)->usize { s.trim().parse().unwrap_or(default) }
fn parse_f64(s:&str, default:f64)->f64 { s.trim().parse().unwrap_or(default) }
