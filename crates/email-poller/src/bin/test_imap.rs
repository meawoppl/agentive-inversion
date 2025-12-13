use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use email_poller::imap_client::ImapClient;
use email_poller::service::format_email_filename;

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn prompt_password(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    rpassword::read_password().unwrap_or_else(|_| {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.trim().to_string()
    })
}

fn prompt_yes_no(msg: &str, default: bool) -> bool {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    let input = prompt(&format!("{} {}: ", msg, suffix));
    match input.to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => default,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== IMAP Email Downloader ===\n");

    let server = prompt("IMAP Server [imap.gmail.com]: ");
    let server = if server.is_empty() {
        "imap.gmail.com".to_string()
    } else {
        server
    };

    let email_addr = prompt("Email address: ");
    let password = prompt_password("Password/App Password: ");

    let count = prompt("Number of emails to fetch [10]: ");
    let count: u32 = count.parse().unwrap_or(10);

    let output_dir = prompt("Output directory [./emails]: ");
    let output_dir = if output_dir.is_empty() {
        PathBuf::from("./emails")
    } else {
        PathBuf::from(output_dir)
    };

    // Create output directory
    fs::create_dir_all(&output_dir)?;
    println!("\nSaving emails to: {}", output_dir.display());

    println!("Connecting to {}...", server);

    let mut client = ImapClient::connect(&server, &email_addr, &password).await?;

    println!("Connected! Fetching {} recent emails...\n", count);

    let emails = client.fetch_recent_emails(count).await?;

    println!("Found {} emails, downloading...\n", emails.len());
    println!("{:-<80}", "");

    let mut downloaded_uids: Vec<u32> = Vec::new();

    for (i, email) in emails.iter().enumerate() {
        let uid = &email.id;
        let uid_num: u32 = uid.parse().unwrap_or(0);

        // Filename format: yymmdd_hhmmss-email-uid.json
        let filename = format_email_filename(email.received_at, &email_addr, uid);
        let filepath = output_dir.join(&filename);

        // Build email data structure
        let email_data = serde_json::json!({
            "uid": uid,
            "mailbox": email_addr,
            "subject": email.subject,
            "from": email.from,
            "received_at": email.received_at,
            "snippet": email.snippet,
            "body": email.body,
        });

        // Write to file
        let json = serde_json::to_string_pretty(&email_data)?;
        fs::write(&filepath, &json)?;

        println!("{}. [UID {}] {}", i + 1, uid, email.subject);
        println!("   From: {}", email.from);
        println!("   Saved: {}", filename);

        downloaded_uids.push(uid_num);
    }

    println!("\n{:-<80}", "");
    println!(
        "Downloaded {} emails to {}",
        emails.len(),
        output_dir.display()
    );

    // Ask about archiving
    if !downloaded_uids.is_empty() {
        println!();
        if prompt_yes_no("Archive these emails from INBOX?", false) {
            println!("Archiving {} emails...", downloaded_uids.len());
            client.archive_many(&downloaded_uids).await?;
            println!("Done! Emails moved out of INBOX (still in All Mail).");
        }
    }

    client.logout().await?;

    Ok(())
}
