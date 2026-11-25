use serde_json::Value as J;
use swirl_lua::Host;

pub const DEBUG_LUA: &str = include_str!("../../swirl-lua/data/debug.lua");

struct Scan {
    long: Option<usize>,
    string: Option<char>,
    depth: usize,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn long_open(chars: &[char], i: usize) -> Option<(usize, usize)> {
    if chars.get(i) != Some(&'[') {
        return None;
    }
    let mut j = i + 1;
    while chars.get(j) == Some(&'=') {
        j += 1;
    }
    if chars.get(j) == Some(&'[') {
        Some((j - i - 1, j + 1 - i))
    } else {
        None
    }
}

fn scan_line(line: &str, st: &mut Scan) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(eq) = st.long {
            let mut close = String::from("]");
            close.push_str(&"=".repeat(eq));
            close.push(']');
            let cc: Vec<char> = close.chars().collect();
            let mut found = None;
            let mut k = i;
            while k + cc.len() <= chars.len() {
                if chars[k..k + cc.len()] == cc[..] {
                    found = Some(k);
                    break;
                }
                k += 1;
            }
            match found {
                None => return out,
                Some(j) => {
                    i = j + cc.len();
                    st.long = None;
                    continue;
                }
            }
        }
        let c = chars[i];
        if let Some(q) = st.string {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == q {
                st.string = None;
            }
            i += 1;
            continue;
        }
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            if let Some((eq, len)) = long_open(&chars, i + 2) {
                st.long = Some(eq);
                i += 2 + len;
                continue;
            }
            return out;
        }
        if c == '[' {
            if let Some((eq, len)) = long_open(&chars, i) {
                st.long = Some(eq);
                out.push('S');
                i += len;
                continue;
            }
        }
        if c == '"' || c == '\'' {
            st.string = Some(c);
            out.push('S');
            i += 1;
            continue;
        }
        if c == '(' || c == '{' || c == '[' {
            st.depth += 1;
        }
        if c == ')' || c == '}' || c == ']' {
            st.depth = st.depth.saturating_sub(1);
        }
        out.push(c);
        i += 1;
    }
    st.string = None;
    out
}

fn leading_word(s: &str) -> &str {
    let end = s.char_indices().find(|(_, c)| !is_word(*c)).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}

fn cont_end(s: &str) -> bool {
    let t = s.trim_end();
    let last = match t.chars().last() {
        Some(c) => c,
        None => return false,
    };
    if "-+*/^%=<>~,([{".contains(last) || t.ends_with("..") {
        return true;
    }
    for w in ["and", "or", "not", "local", "function", "return"] {
        if t.ends_with(w) {
            let before = &t[..t.len() - w.len()];
            if !before.chars().last().map(is_word).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

fn block_end(code: &str) -> bool {
    matches!(leading_word(code), "end" | "else" | "elseif" | "until")
}

fn return_break(s: &str) -> bool {
    matches!(leading_word(s), "return" | "break")
}

fn starts_op(code: &str, with_paren: bool) -> bool {
    match code.chars().next() {
        Some(c) => ".:+-*/^%=<>~,#".contains(c) || (with_paren && (c == '(' || c == '[')),
        None => false,
    }
}

fn no_before(first: &str) -> bool {
    matches!(first, "and" | "or" | "not" | "in" | "then" | "do")
}

pub fn instrument(source: &str, sid: &str) -> String {
    let lines: Vec<&str> = source.split('\n').collect();
    let mut st = Scan { long: None, string: None, depth: 0 };
    let mut prev_code = String::new();
    let mut prev_start = String::new();
    let mut out: Vec<String> = vec![];
    let sid_json = serde_json::to_string(sid).unwrap_or_default();
    for (n, line) in lines.iter().enumerate() {
        let start_long = st.long.is_some();
        let start_depth = st.depth;
        let code = scan_line(line, &mut st).trim().to_string();
        let mut ok = !start_long && start_depth == 0 && !code.is_empty();
        if ok && (code.starts_with('(') || code.starts_with('[')) {
            ok = false;
        }
        if ok && !prev_code.is_empty() && cont_end(&prev_code) {
            ok = false;
        }
        if ok && block_end(&code) && return_break(&prev_start) {
            ok = false;
        }
        if ok && return_break(&prev_start) && !block_end(&code) {
            ok = false;
        }
        let first: String = code.chars().take_while(|c| c.is_ascii_alphabetic() || *c == '_').collect();
        if ok && no_before(&first) {
            ok = false;
        }
        if ok && starts_op(&code, false) {
            ok = false;
        }
        if ok {
            out.push(format!("__dbg({},{}) {}", sid_json, n + 1, line));
        } else {
            out.push(line.to_string());
        }
        if !code.is_empty() {
            let continuation = start_long || start_depth > 0 || (!prev_code.is_empty() && cont_end(&prev_code)) || no_before(&first) || starts_op(&code, true);
            if !continuation {
                prev_start = code.clone();
            }
            prev_code = code;
        }
    }
    out.join("\n")
}

fn near_dbg_line(err: &str) -> Option<usize> {
    let b = err.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b':' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && b.get(j) == Some(&b':') && err[j + 1..].contains("near '__dbg'") {
                return err[i + 1..j].parse().ok();
            }
        }
        i += 1;
    }
    None
}

pub fn instrument_checked(source: &str, sid: &str, compile: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut lines: Vec<String> = instrument(source, sid).split('\n').map(|s| s.to_string()).collect();
    let prefix = format!("__dbg({},", serde_json::to_string(sid).unwrap_or_default());
    for _ in 0..200 {
        let joined = lines.join("\n");
        let err = match compile(&joined) {
            None => return Some(joined),
            Some(e) => e,
        };
        let n = near_dbg_line(&err)? - 1;
        let l = lines.get(n)?;
        if !l.starts_with(&prefix) {
            return None;
        }
        let cut = l.find(") ")? + 2;
        lines[n] = l[cut..].to_string();
    }
    None
}

pub struct DebugAgent;

impl DebugAgent {
    pub fn attach(host: &mut Host) -> Result<(), String> {
        host.extend(DEBUG_LUA, "debug")?;
        host.debug_attach()
    }

    pub fn handle(host: &mut Host, msg: &J) {
        let cmd = match msg["cmd"].as_str() {
            Some(c) => c,
            None => return,
        };
        match cmd {
            "bps" => {
                let mut parts: Vec<String> = vec![];
                if let Some(o) = msg["bps"].as_object() {
                    for (sid, list) in o {
                        let nums: Vec<String> = list.as_array().map(|a| a.iter().map(js_str).collect()).unwrap_or_default();
                        parts.push(format!("{}={}", sid, nums.join(",")));
                    }
                }
                host.debug_command("bps", &parts.join(";"));
            }
            "watches" => {
                let ws: Vec<String> = msg["watches"].as_array().map(|a| a.iter().map(|w| js_str(w).replace('\n', " ")).collect()).unwrap_or_default();
                host.debug_command("watches", &ws.join("\n"));
            }
            "continue" | "into" | "over" | "out" | "pause" => host.debug_command(cmd, ""),
            _ => {}
        }
    }

    pub fn drain(host: &mut Host) -> Vec<J> {
        host.take_debug()
            .into_iter()
            .map(|s| serde_json::from_str::<J>(&s).unwrap_or_else(|_| serde_json::json!({"error": "bad pause data"})))
            .collect()
    }
}

fn js_str(v: &J) -> String {
    match v {
        J::String(s) => s.clone(),
        J::Null => "null".to_string(),
        o => o.to_string(),
    }
}
