//! Times old vs new first-sync strategy. See src/devtools.rs.
fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    if let Err(e) = rt.block_on(pulse_mail_lib::devtools::bench()) {
        eprintln!("bench failed: {e}");
        std::process::exit(1);
    }
}
