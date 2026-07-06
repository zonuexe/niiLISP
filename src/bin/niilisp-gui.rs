//! `niilisp-gui` — the native GUI helper (ADR-0034).
//!
//! Renders with fltk and is driven by niiLISP over one full-duplex TCP socket:
//! it reads command lines (space-separated tokens, text base64-encoded) and
//! emits event lines that are niiLISP source the interpreter `eval-string`s
//! (e.g. `(on-click "B")`). A separate process so the toolkit's event loop owns
//! the main thread. This is an MVP: a vertical auto-layout of window / label /
//! button / text-field with click events; richer layout and widgets are later.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;

use fltk::{
    app, button::Button, enums::Color, frame::Frame, input::Input, prelude::*, window::Window,
};

/// A parsed command line from the interpreter. Only owned strings, so it crosses
/// the reader thread → main thread channel (fltk widgets are not `Send`).
struct Command {
    name: String,
    args: Vec<String>,
}

/// Decode a standard base64 token (the protocol's text encoding).
fn b64_decode(s: &str) -> String {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => continue,
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The kinds of widget we retain by id.
enum W {
    Win(Window),
    Btn(Button),
    Lbl(Frame),
    Inp(Input),
}

struct Gui {
    widgets: HashMap<String, W>,
    /// The window widgets are currently being added to, and its layout cursor.
    current: Option<String>,
    cursor_y: i32,
    width: i32,
    /// The socket write half, for emitting event lines.
    out: TcpStream,
}

impl Gui {
    /// Emit an event line (niiLISP source) back to the interpreter.
    fn emit(out: &mut TcpStream, line: &str) {
        let _ = out.write_all(line.as_bytes());
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    }

    fn next_slot(&mut self) -> (i32, i32, i32, i32) {
        let y = self.cursor_y;
        self.cursor_y += 40;
        (10, y, self.width - 20, 30)
    }

    fn apply(&mut self, cmd: &Command) {
        let a = &cmd.args;
        let id = a.first().cloned().unwrap_or_default();
        match cmd.name.as_str() {
            "frame" | "window" => {
                // frame ID X Y W H B64TITLE
                let n = |i: usize| a.get(i).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                let (x, y, w, h) = (n(1), n(2), n(3), n(4));
                let title = a.get(5).map(|s| b64_decode(s)).unwrap_or_default();
                let mut win = Window::new(x, y, w.max(120), h.max(80), None);
                win.set_label(&title);
                win.begin();
                self.width = w.max(120);
                self.cursor_y = 10;
                self.current = Some(id.clone());
                self.widgets.insert(id, W::Win(win));
            }
            "button" => {
                // button ID ACTION B64TEXT
                let action = a.get(1).cloned().unwrap_or_default();
                let text = a.get(2).map(|s| b64_decode(s)).unwrap_or_default();
                let (x, y, w, h) = self.next_slot();
                let mut b = Button::new(x, y, w, h, None);
                b.set_label(&text);
                let mut out = self.out.try_clone().expect("clone socket");
                let bid = id.clone();
                b.set_callback(move |_| {
                    Gui::emit(&mut out, &format!("({} \"{}\")", action, bid));
                });
                self.widgets.insert(id, W::Btn(b));
            }
            "label" => {
                let text = a.get(1).map(|s| b64_decode(s)).unwrap_or_default();
                let (x, y, w, h) = self.next_slot();
                let mut f = Frame::new(x, y, w, h, None);
                f.set_label(&text);
                self.widgets.insert(id, W::Lbl(f));
            }
            "text-field" => {
                let action = a.get(1).cloned().unwrap_or_default();
                let (x, y, w, h) = self.next_slot();
                let mut inp = Input::new(x, y, w, h, None);
                if !action.is_empty() && action != "nil" {
                    let mut out = self.out.try_clone().expect("clone socket");
                    let iid = id.clone();
                    inp.set_callback(move |w| {
                        Gui::emit(
                            &mut out,
                            &format!("({} \"{}\" \"{}\")", action, iid, w.value()),
                        );
                    });
                }
                self.widgets.insert(id, W::Inp(inp));
            }
            "set-text" => {
                let text = a.get(1).map(|s| b64_decode(s)).unwrap_or_default();
                match self.widgets.get_mut(&id) {
                    Some(W::Lbl(f)) => f.set_label(&text),
                    Some(W::Btn(b)) => b.set_label(&text),
                    Some(W::Inp(i)) => i.set_value(&text),
                    Some(W::Win(w)) => w.set_label(&text),
                    None => {}
                }
                app::redraw();
            }
            "set-background" => {
                // set-background ID R G B
                let n = |i: usize| a.get(i).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
                let color = Color::from_rgb(n(1), n(2), n(3));
                if let Some(w) = self.widgets.get_mut(&id) {
                    match w {
                        W::Win(x) => x.set_color(color),
                        W::Btn(x) => x.set_color(color),
                        W::Lbl(x) => x.set_color(color),
                        W::Inp(x) => x.set_color(color),
                    }
                }
                app::redraw();
            }
            "set-visible" => {
                // set-visible ID FLAG — showing a window ends its layout group.
                let show = a.get(1).map(|s| s != "0").unwrap_or(true);
                if let Some(W::Win(win)) = self.widgets.get_mut(&id) {
                    win.end();
                    if show {
                        win.show();
                    } else {
                        win.hide();
                    }
                }
            }
            "add-to" => {} // implicit under fltk's begin/end grouping
            _ => {}
        }
    }
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(64010);
    let stream = match TcpStream::connect(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("niilisp-gui: cannot connect to niiLISP on {}: {}", port, e);
            std::process::exit(1);
        }
    };
    let out = stream.try_clone().expect("clone socket");
    let reader = BufReader::new(stream);

    let app = app::App::default();
    let (tx, rx) = mpsc::channel::<Command>();

    // Read command lines off the socket on a side thread; wake the fltk loop.
    std::thread::spawn(move || {
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let mut it = line.split_whitespace();
            let name = match it.next() {
                Some(n) => n.to_string(),
                None => continue,
            };
            let args = it.map(str::to_string).collect();
            if tx.send(Command { name, args }).is_err() {
                break;
            }
            app::awake();
        }
        app::awake();
    });

    let mut gui = Gui {
        widgets: HashMap::new(),
        current: None,
        cursor_y: 10,
        width: 300,
        out,
    };

    while app.wait() {
        while let Ok(cmd) = rx.try_recv() {
            gui.apply(&cmd);
        }
    }
}
