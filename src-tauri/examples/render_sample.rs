//! Render an HTML email through the sanitizer and write the frame document.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let dark = args.get(3).is_some_and(|a| a == "dark");
    let doc = pulse_mail_lib::devtools::render_file(std::path::Path::new(input), dark).expect("render");
    std::fs::write(output, doc).expect("write");
}
