use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use clap::{Parser, Subcommand};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::str;

const HIRAGANA_START: u32 = 0x3041; // 'ぁ'
const BASE64_LEN: u32 = 64; // base64url indices 0..63

#[derive(Debug, Serialize, Deserialize)]
struct Member {
    name: String,
    count: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct Book {
    people: usize,
    interval: usize,
    members: Vec<Member>,
}

#[derive(Parser)]
#[command(
    name = "touban",
    about = "とうばんのしょ CLI (hiragana single-line state)"
)]

struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new とうばんのしょ
    Create {
        /// How many people to assign each time
        #[arg(long)]
        people: usize,
        /// Interval in days
        #[arg(long)]
        interval: usize,
        /// Comma-separated member names, e.g. "たろう,はなこ,じろう"
        #[arg(long)]
        members: Option<String>,
    },
    /// Show the contents of a とうばんのしょ (pass the hiragana string)
    Show {
        #[arg(long)]
        book: String,
    },
    /// Add a member (returns updated とうばんのしょ)
    AddMember {
        #[arg(long)]
        book: String,
        #[arg(long)]
        member: String,
    },
    /// Remove a member (returns updated とうばんのしょ)
    RemoveMember {
        #[arg(long)]
        book: String,
        #[arg(long)]
        member: String,
    },
    /// Assign this period (returns selected members + updated とうばんのしょ)
    Assign {
        #[arg(long)]
        book: String,
        /// Optional deterministic seed (u64) to control randomness
        #[arg(long)]
        seed: Option<u64>,
    },
}

// --------------------- Base64URL <-> Hiragana (one-shot mapping) ---------------------
fn base64url_char_to_hiragana(ch: char) -> Option<char> {
    // map base64url char -> index 0..63
    let idx = match ch as u8 {
        b'A'..=b'Z' => (ch as u8 - b'A') as u32,      // 0..25
        b'a'..=b'z' => (ch as u8 - b'a') as u32 + 26, // 26..51
        b'0'..=b'9' => (ch as u8 - b'0') as u32 + 52, // 52..61
        b'-' => 62,
        b'_' => 63,
        _ => return None,
    };
    let cp = HIRAGANA_START + (idx % BASE64_LEN);
    std::char::from_u32(cp)
}

fn hiragana_char_to_base64url(ch: char) -> Option<char> {
    let cp = ch as u32;
    if cp < HIRAGANA_START || cp >= HIRAGANA_START + BASE64_LEN {
        return None;
    }
    let idx = cp - HIRAGANA_START; // 0..63
    match idx {
        0..=25 => std::char::from_u32((b'A' + idx as u8) as u32),
        26..=51 => std::char::from_u32((b'a' + (idx as u8 - 26)) as u32),
        52..=61 => std::char::from_u32((b'0' + (idx as u8 - 52)) as u32),
        62 => Some('-'),
        63 => Some('_'),
        _ => None,
    }
}

fn base64url_to_hiragana(b64: &str) -> Result<String> {
    let mut out = String::with_capacity(b64.len());
    for ch in b64.chars() {
        let hira = base64url_char_to_hiragana(ch)
            .ok_or_else(|| anyhow!("invalid base64url char encountered: {:?}", ch))?;
        out.push(hira);
    }
    Ok(out)
}

fn hiragana_to_base64url(hira: &str) -> Result<String> {
    let mut out = String::with_capacity(hira.chars().count());
    for ch in hira.chars() {
        let b = hiragana_char_to_base64url(ch)
            .ok_or_else(|| anyhow!("invalid hiragana char encountered: {:?}", ch))?;
        out.push(b);
    }
    Ok(out)
}

// --------------------- Encode / Decode Book ---------------------
fn encode_book(book: &Book) -> Result<String> {
    let json = serde_json::to_vec(book).context("serialize book to json")?;
    let b64 = URL_SAFE_NO_PAD.encode(&json);
    base64url_to_hiragana(&b64)
}

fn decode_book(hira: &str) -> Result<Book> {
    let b64 = hiragana_to_base64url(hira)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(&b64)
        .context("base64url decode failed; maybe corrupted とうばんのしょ")?;
    let book = serde_json::from_slice::<Book>(&bytes).context("json decode failed")?;
    Ok(book)
}

// --------------------- Utilities ---------------------
fn split_members_arg(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

// --------------------- Command Implementations ---------------------
fn cmd_create(people: usize, interval: usize, members: Option<String>) -> Result<()> {
    if people == 0 {
        return Err(anyhow!("--people must be >= 1"));
    }
    let members_vec = members.map(|s| split_members_arg(&s)).unwrap_or_default();
    let members_struct = members_vec
        .into_iter()
        .map(|name| Member { name, count: 0 })
        .collect::<Vec<_>>();
    let book = Book {
        people,
        interval,
        members: members_struct,
    };
    let hira = encode_book(&book)?;
    println!(":桜: あたらしい とうばんのしょ が できました。");
    println!("{}", hira);
    Ok(())
}

fn cmd_show(book_str: String) -> Result<()> {
    let book = decode_book(&book_str)?;
    println!(":本: とうばんのしょ の なかみ：");
    println!(":上半身シルエット_2: とうばん人数: {}", book.people);
    println!(":リピート: 間隔（日）: {}", book.interval);
    println!(":上半身シルエット_1: メンバー一覧:");
    for m in &book.members {
        println!(" - {} ({}回)", m.name, m.count);
    }
    Ok(())
}

fn cmd_add_member(book_str: String, member: String) -> Result<()> {
    let mut book = decode_book(&book_str)?;
    if book.members.iter().any(|m| m.name == member) {
        return Err(anyhow!("メンバー「{}」は既に存在します", member));
    }
    let avg = if book.members.is_empty() {
        0
    } else {
        let s: usize = book.members.iter().map(|m| m.count as usize).sum();
        ((s as f64) / (book.members.len() as f64)).round() as u8
    };
    book.members.push(Member {
        name: member,
        count: avg as u8,
    });
    let hira = encode_book(&book)?;
    println!(":上半身シルエット_1: メンバーを追加しました。");
    println!("{}", hira);
    Ok(())
}

fn cmd_remove_member(book_str: String, member: String) -> Result<()> {
    let mut book = decode_book(&book_str)?;
    let before = book.members.len();
    book.members.retain(|m| m.name != member);
    if book.members.len() == before {
        return Err(anyhow!("メンバー「{}」は見つかりませんでした", member));
    }
    let hira = encode_book(&book)?;
    println!(":ハロー: メンバーを削除しました。");
    println!("{}", hira);
    Ok(())
}

fn cmd_assign(book_str: String, seed: Option<u64>) -> Result<()> {
    let mut book = decode_book(&book_str)?;
    if book.members.is_empty() {
        return Err(anyhow!("メンバーがいません"));
    }
    // reset when any count >= 5
    if book.members.iter().map(|m| m.count).max().unwrap_or(0) >= 5 {
        for m in &mut book.members {
            m.count = 0;
        }
        println!(":反時計回り矢印: 全員のカウントをリセットしました。");
    }
    // find min count
    let minc = book.members.iter().map(|m| m.count).min().unwrap_or(0);
    // collect candidates (by index to later update counts)
    let mut candidates_idx: Vec<usize> = book
        .members
        .iter()
        .enumerate()
        .filter(|(_, m)| m.count == minc)
        .map(|(i, _)| i)
        .collect();
    // shuffle (deterministic if seed given)
    if let Some(s) = seed {
        let mut rng = ChaCha8Rng::seed_from_u64(s);
        candidates_idx.shuffle(&mut rng);
    } else {
        let mut rng = thread_rng();
        candidates_idx.shuffle(&mut rng);
    }
    let take = min(book.people, candidates_idx.len());
    let selected_idx = &candidates_idx[0..take];
    println!(":ダーツ: 今週のとうばん：");
    for &i in selected_idx {
        // increment count with wrap >5 -> 0
        let newc = book.members[i].count.saturating_add(1);
        book.members[i].count = if newc > 5 { 0 } else { newc };
        println!(
            " - {} ({}回め)",
            book.members[i].name, book.members[i].count
        );
    }
    let hira = encode_book(&book)?;
    println!("\n:青い本: とうばんのしょ（更新後）:");
    println!("{}", hira);
    Ok(())
}

// --------------------- main ---------------------
fn main() -> Result<()> {
    let cli = Cli::parse();
    let res = match cli.cmd {
        Commands::Create {
            people,
            interval,
            members,
        } => cmd_create(people, interval, members),
        Commands::Show { book } => cmd_show(book),
        Commands::AddMember { book, member } => cmd_add_member(book, member),
        Commands::RemoveMember { book, member } => cmd_remove_member(book, member),
        Commands::Assign { book, seed } => cmd_assign(book, seed),
    };
    if let Err(e) = res {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}
