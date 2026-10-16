//! Prints the forms in an NE executable as text DFMs.
//!
//! `cargo run -p formats --example dfmdump -- orig/HOLIDAY.EXE [FORMCLASS]`

use formats::dfm::{Object, Value};
use formats::ne::{RT_RCDATA, ResId, resources};

fn text(s: &[u8]) -> String {
    s.iter().map(|&c| char::from(c)).collect()
}

fn value(v: &Value, indent: usize) -> String {
    match v {
        Value::List(items) => {
            let pad = "  ".repeat(indent + 1);
            let items: Vec<_> = items
                .iter()
                .map(|i| format!("\n{pad}{}", value(i, indent + 1)))
                .collect();
            format!("({})", items.concat())
        }
        Value::Int(n) => n.to_string(),
        Value::Extended(x) => format!("<extended {x:02x?}>"),
        Value::String(s) => format!("'{}'", text(s).replace('\'', "''")),
        Value::Ident(s) => text(s),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_owned(),
        Value::Binary(b) => format!("{{{} bytes}}", b.len()),
        Value::Set(m) => format!("[{}]", m.iter().map(|s| text(s)).collect::<Vec<_>>().join(", ")),
    }
}

fn dump(o: &Object, indent: usize) {
    let pad = "  ".repeat(indent);
    println!("{pad}object {}: {}", text(&o.name), text(&o.class));
    for (name, v) in &o.properties {
        println!("{pad}  {} = {}", text(name), value(v, indent + 1));
    }
    for child in &o.children {
        dump(child, indent + 1);
    }
    println!("{pad}end");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: dfmdump EXE [FORMCLASS]");
    let only = args.next().map(|s| s.to_ascii_uppercase().into_bytes());
    let exe = std::fs::read(&path).expect("can't read the executable");
    for res in resources(&exe).expect("not an NE executable") {
        let ResId::Name(name) = &res.name else { continue };
        if res.kind != ResId::Id(RT_RCDATA) || only.as_ref().is_some_and(|o| o != name) {
            continue;
        }
        match Object::parse(res.data) {
            Ok(form) => dump(&form, 0),
            Err(e) => eprintln!("{}: {e}", text(name)),
        }
    }
}
