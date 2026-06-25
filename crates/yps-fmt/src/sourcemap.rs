const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn vlq_encode(n: i64) -> String {
    let mut vlq: u64 = if n >= 0 { (n as u64) << 1 } else { (((-n) as u64) << 1) | 1 };
    let mut out = String::new();
    loop {
        let digit = (vlq & 0x1f) as u8;
        vlq >>= 5;
        if vlq > 0 {
            out.push(BASE64[(digit | 0x20) as usize] as char);
        } else {
            out.push(BASE64[digit as usize] as char);
            break;
        }
    }
    out
}

#[derive(Debug)]
pub struct Mapping {
    pub gen_line: u32,
    pub gen_col: u32,
    pub src_line: u32,
    pub src_col: u32,
}

pub struct SourceMapBuilder {
    source: String,
    mappings: Vec<Mapping>,
}

impl SourceMapBuilder {
    pub fn new(source: &str) -> Self {
        Self { source: source.to_string(), mappings: Vec::new() }
    }

    pub fn add_mapping(&mut self, gen_line: u32, gen_col: u32, src_byte: usize) {
        let (src_line, src_col) = byte_to_lc(&self.source, src_byte);
        self.mappings.push(Mapping { gen_line, gen_col, src_line, src_col });
    }

    pub fn build(self, file: &str, source_name: &str) -> SourceMap {
        SourceMap {
            file: file.to_string(),
            source_name: source_name.to_string(),
            source: self.source,
            mappings: self.mappings,
        }
    }
}

#[derive(Debug)]
pub struct SourceMap {
    pub file: String,
    pub source_name: String,
    pub source: String,
    pub mappings: Vec<Mapping>,
}

impl SourceMap {
    pub fn to_json(&self) -> String {
        let encoded = encode_mappings(&self.mappings);
        let file = json_escape_str(&self.file);
        let source_name = json_escape_str(&self.source_name);
        let source_content = json_escape_str(&self.source);
        format!(
            "{{\"version\":3,\"file\":\"{file}\",\"sources\":[\"{source_name}\"],\"sourcesContent\":[\"{source_content}\"],\"mappings\":\"{encoded}\"}}",
        )
    }
}

fn json_escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn encode_mappings(mappings: &[Mapping]) -> String {
    if mappings.is_empty() {
        return String::new();
    }

    let max_line = mappings.iter().map(|m| m.gen_line).max().unwrap_or(0);
    let mut lines: Vec<Vec<&Mapping>> = vec![Vec::new(); (max_line + 1) as usize];
    for m in mappings {
        lines[m.gen_line as usize].push(m);
    }

    let mut prev_src_line: i64 = 0;
    let mut prev_src_col: i64 = 0;
    let mut result = String::new();

    for (li, segs) in lines.iter().enumerate() {
        if li > 0 {
            result.push(';');
        }
        let mut prev_gen_col: i64 = 0;
        for (si, m) in segs.iter().enumerate() {
            if si > 0 {
                result.push(',');
            }
            let gen_col_delta = m.gen_col as i64 - prev_gen_col;
            prev_gen_col = m.gen_col as i64;

            let src_line_delta = m.src_line as i64 - prev_src_line;
            prev_src_line = m.src_line as i64;

            let src_col_delta = m.src_col as i64 - prev_src_col;
            prev_src_col = m.src_col as i64;

            result.push_str(&vlq_encode(gen_col_delta));
            result.push('A');
            result.push_str(&vlq_encode(src_line_delta));
            result.push_str(&vlq_encode(src_col_delta));
        }
    }

    result
}

fn byte_to_lc(src: &str, offset: usize) -> (u32, u32) {
    let clamped = (0..=offset.min(src.len())).rev().find(|&i| src.is_char_boundary(i)).unwrap_or(0);
    let prefix = &src[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = prefix[line_start..].encode_utf16().count() as u32;
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_known_values() {
        assert_eq!(vlq_encode(0), "A");
        assert_eq!(vlq_encode(1), "C");
        assert_eq!(vlq_encode(-1), "D");
        assert_eq!(vlq_encode(16), "gB");
        assert_eq!(vlq_encode(-16), "hB");
        assert_eq!(vlq_encode(15), "e");
    }

    #[test]
    fn byte_to_lc_basic() {
        let src = "гыы х = 1;\nгыы у = 2;";
        assert_eq!(byte_to_lc(src, 0), (0, 0));
        let second_line_start = src.find('\n').unwrap() + 1;
        assert_eq!(byte_to_lc(src, second_line_start), (1, 0));
    }

    #[test]
    fn source_map_json_is_valid() {
        let mut builder = SourceMapBuilder::new("гыы х = 1;");
        builder.add_mapping(0, 0, 0);
        let map = builder.build("out.yopta", "in.yopta");
        let json = map.to_json();
        assert!(json.contains("\"version\":3"));
        assert!(json.contains("\"sources\":[\"in.yopta\"]"));
        assert!(json.contains("\"mappings\":"));
    }

    #[test]
    fn json_escapes_control_characters() {
        let src = "гыы х = 1;\n\tотвечаю 2;\r\n";
        let builder = SourceMapBuilder::new(src);
        let map = builder.build("out.yopta", "in.yopta");
        let json = map.to_json();
        assert!(!json.contains('\t'), "raw tab must be escaped");
        assert!(!json.contains('\r'), "raw CR must be escaped");
        assert!(json.contains("\\t"));
        assert!(json.contains("\\r"));
        assert!(json.contains("\\n"));
    }

    #[test]
    fn encode_mappings_single_segment() {
        let mappings = vec![Mapping { gen_line: 0, gen_col: 0, src_line: 0, src_col: 0 }];
        let encoded = encode_mappings(&mappings);
        assert!(!encoded.is_empty());
        assert!(!encoded.contains(';'), "single line should have no semicolons");
    }

    #[test]
    fn encode_mappings_two_lines() {
        let mappings = vec![
            Mapping { gen_line: 0, gen_col: 0, src_line: 0, src_col: 0 },
            Mapping { gen_line: 1, gen_col: 0, src_line: 1, src_col: 0 },
        ];
        let encoded = encode_mappings(&mappings);
        assert!(encoded.contains(';'), "two lines need a semicolon separator");
    }
}
