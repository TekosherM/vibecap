//! Lightweight desktop notifications (no extra crates).
//!
//! Used when an agent posts a HITL question so the human notices even if
//! Vibecap is hidden in the menu bar.

use std::process::Command;

/// Fire a system notification. Best-effort; never panics.
pub fn notify_desktop(title: &str, body: &str) {
    let title = sanitize(title, 80);
    let body = sanitize(body, 180);
    if title.is_empty() && body.is_empty() {
        return;
    }

    #[cfg(target_os = "macos")]
    {
        // osascript is always available; no TCC prompt for display notification.
        let t = applescript_escape(&title);
        let b = applescript_escape(&body);
        let script = format!(
            r#"display notification "{b}" with title "{t}" sound name "Glass""#
        );
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        // notify-send if present (libnotify).
        let _ = Command::new("notify-send")
            .args(["-a", "Vibecap", &title, &body])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        // PowerShell balloon tip — works without extra packages.
        let t = powershell_escape(&title);
        let b = powershell_escape(&body);
        let ps = format!(
            r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
$template.GetElementsByTagName('text').Item(0).AppendChild($template.CreateTextNode('{t}')) > $null; \
$template.GetElementsByTagName('text').Item(1).AppendChild($template.CreateTextNode('{b}')) > $null; \
$toast = [Windows.UI.Notifications.ToastNotification]::new($template); \
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Vibecap').Show($toast)"#
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// Agent HITL convenience: title + truncated question.
pub fn notify_agent_question(agent_label: &str, question: &str, priority: &str) {
    let agent = if agent_label.trim().is_empty() {
        "Agent"
    } else {
        agent_label.trim()
    };
    let title = if priority.eq_ignore_ascii_case("high") {
        format!("Vibecap · {agent} (high)")
    } else {
        format!("Vibecap · {agent}")
    };
    let body = if question.trim().is_empty() {
        "Needs your answer in Inbox".into()
    } else {
        sanitize(question, 160)
    };
    notify_desktop(&title, &body);
}

fn sanitize(s: &str, max_chars: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() <= max_chars {
        flat
    } else {
        let mut out: String = flat.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
fn powershell_escape(s: &str) -> String {
    s.replace('\'', "''")
}
