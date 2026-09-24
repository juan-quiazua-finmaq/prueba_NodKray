//! TTY helpers for `nodkray init`. Non-interactive callers never hit these.

use std::io::{self, IsTerminal, Write};

use crate::error::{NodkrayError, NodkrayResult};

pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub fn prompt_line(message: &str, default: &str) -> NodkrayResult<String> {
    eprint!("{message}");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|err| NodkrayError::user_input("INIT_INPUT_ERROR", err.to_string()))?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn prompt_yes_no(message: &str, default_yes: bool) -> NodkrayResult<bool> {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    let answer = prompt_line(&format!("{message} [{hint}]: "), "")?;
    Ok(match answer.to_ascii_lowercase().as_str() {
        "" => default_yes,
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default_yes,
    })
}

pub fn prompt_choice(label: &str, options: &[String], default: &str) -> NodkrayResult<String> {
    if options.is_empty() {
        return Ok(default.to_string());
    }
    eprintln!("{label}");
    for (idx, option) in options.iter().enumerate() {
        let marker = if option == default { "*" } else { " " };
        eprintln!("  {marker} [{}] {option}", idx + 1);
    }
    let raw = prompt_line(&format!("choice (default {default}): "), default)?;
    if let Ok(n) = raw.parse::<usize>() {
        if let Some(choice) = options.get(n.saturating_sub(1)) {
            return Ok(choice.clone());
        }
    }
    if options.iter().any(|o| o == &raw) {
        return Ok(raw);
    }
    Ok(default.to_string())
}
