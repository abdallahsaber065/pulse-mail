//! Developer-only helpers (not used by the app). `cargo run --example
//! sync_bench` times the old vs new first-sync strategy against the signed-in
//! account. Read-only Gmail calls; prints only counts and timings.

use crate::auth::{TokenManager, client_config, token_store};
use crate::sync::gmail::Gmail;
use futures::StreamExt;
use std::time::Instant;

pub async fn bench() -> Result<(), String> {
    let _ = tracing_subscriber::fmt().with_writer(std::io::stderr).try_init();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set")?;
    let dir = std::path::Path::new(&appdata).join("com.abdallah.pulsemail");
    let client = client_config::load(&client_config::resolve_path(&dir)).map_err(|e| e.to_string())?;
    let conn =
        rusqlite::Connection::open_with_flags(dir.join("pulse-mail.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
    let email: String = conn
        .query_row("SELECT email FROM accounts ORDER BY id LIMIT 1", [], |r| r.get(0))
        .map_err(|_| "no signed-in account")?;
    drop(conn);
    let refresh = token_store::load_refresh_token(&email)
        .map_err(|e| e.to_string())?
        .ok_or("no refresh token")?;
    let http = reqwest::Client::builder()
        .https_only(true)
        .build()
        .map_err(|e| e.to_string())?;
    let tm = TokenManager::new(email, client, http.clone(), refresh);
    let token = tm.access_token().await.map_err(|e| e.to_string())?;
    let gmail = Gmail::new(http);

    // NEW: profile + INBOX list in parallel, then the 50 newest as metadata.
    let t = Instant::now();
    let (_, refs) = tokio::try_join!(
        gmail.profile(&token),
        gmail.list_message_ids(&token, Some("INBOX"), None, 50)
    )
    .map_err(|e| e.to_string())?;
    let listed = t.elapsed();
    let ids: Vec<String> = refs.iter().map(|r| r.id.clone()).collect();
    println!("list INBOX x{}: {:.2}s", ids.len(), listed.as_secs_f64());

    for conc in [10usize, 25] {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let t = Instant::now();
        let mut s = futures::stream::iter(ids.iter().cloned())
            .map(|id| {
                let token = token.clone();
                let gmail = &gmail;
                async move { gmail.get_metadata(&token, &id).await.is_ok() }
            })
            .buffered(conc);
        let (mut n, mut ok, mut first12) = (0usize, 0usize, None);
        while let Some(r) = s.next().await {
            n += 1;
            ok += r as usize;
            if n == 12 {
                first12 = Some(t.elapsed());
            }
        }
        println!(
            "NEW metadata single @{conc}: {ok}/{n} ok, first 12 {:.2}s, all 50 {:.2}s (+list)",
            first12.unwrap_or_default().as_secs_f64(),
            t.elapsed().as_secs_f64()
        );
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let t = Instant::now();
    let r = gmail
        .get_metadata_batch(&token, &ids)
        .await
        .map_err(|e| e.to_string())?;
    let ok = r.iter().filter(|x| x.is_ok()).count();
    let codes: Vec<u16> = r.iter().filter_map(|x| x.as_ref().err().copied()).collect();
    println!(
        "NEW metadata batch x{}: {ok} ok {codes:?}, one round trip {:.2}s (+list)",
        ids.len(),
        t.elapsed().as_secs_f64()
    );

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // OLD (v0.1.0): list 300 from the last 60 days, fetch each as raw with 6
    // in flight; the UI refreshed only after the whole batch.
    let t = Instant::now();
    let refs = gmail
        .list_message_ids(&token, None, Some("newer_than:60d -in:spam -in:trash"), 300)
        .await
        .map_err(|e| e.to_string())?;
    let listed = t.elapsed();
    let mut s = futures::stream::iter(refs.iter().map(|r| r.id.clone()))
        .map(|id| {
            let token = token.clone();
            let gmail = &gmail;
            async move { gmail.get_raw(&token, &id).await.map(|r| r.raw.len()) }
        })
        .buffer_unordered(6);
    let (mut n, mut bytes, mut first50) = (0usize, 0usize, None);
    while let Some(r) = s.next().await {
        if let Ok(b) = r {
            bytes += b;
        }
        n += 1;
        if n == 50 {
            first50 = Some(t.elapsed());
        }
    }
    let old_total = t.elapsed();
    println!(
        "OLD raw x{n}: list {:.2}s, 50 fetched {:.2}s, all fetched (= first rows in UI) {:.2}s, {:.1} MB",
        listed.as_secs_f64(),
        first50.unwrap_or_default().as_secs_f64(),
        old_total.as_secs_f64(),
        bytes as f64 / 1e6
    );

    Ok(())
}

/// Render an HTML file through the email pipeline (sanitizer + frame
/// document), for checking the frame in a browser: `cargo run --example
/// render_sample -- in.html out.html [dark]`.
pub fn render_file(input: &std::path::Path, dark: bool) -> std::io::Result<String> {
    let html = std::fs::read_to_string(input)?;
    let opts = crate::render::RenderOptions {
        dark,
        original_colors: false,
    };
    Ok(crate::render::render_html(&html, crate::render::Resources::default(), opts).srcdoc)
}
