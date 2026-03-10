use std::path::Path;

use crate::config::inline::{build_inline_asset, supported_inline_tag};
use crate::errors::AppError;
use crate::profiles::{
    AssetKind, AssetReference, ParsedDirective, ParsedProfile,
};

pub fn parse_profile(source: &str, base_dir: &Path) -> Result<ParsedProfile, AppError> {
    let mut directives = Vec::new();
    let mut referenced_assets = Vec::new();
    let mut inline_assets = Vec::new();
    let mut remotes = Vec::new();
    let mut dns_directives = Vec::new();
    let mut requires_auth_user_pass = false;

    let mut lines = source.lines().enumerate().peekable();
    while let Some((index, raw_line)) = lines.next() {
        let line_no = index + 1;
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(tag) = line.strip_prefix('<').and_then(|value| value.strip_suffix('>')) {
            if supported_inline_tag(tag).is_some() {
                let mut content = String::new();
                let closing = format!("</{tag}>");
                let start_line = line_no;
                loop {
                    let Some((_, inner_line)) = lines.next() else {
                        return Err(AppError::Validation {
                            title: "Invalid inline block".into(),
                            message: format!("Inline block <{tag}> is not closed."),
                            directive: Some(tag.to_string()),
                            line: Some(start_line),
                        });
                    };
                    if inner_line.trim() == closing {
                        break;
                    }
                    content.push_str(inner_line);
                    content.push('\n');
                }
                if let Some(asset) = build_inline_asset(tag, start_line, content.trim_end().to_string()) {
                    inline_assets.push(asset);
                }
                continue;
            }
        }

        let tokens = tokenize_line(line);
        if tokens.is_empty() {
            continue;
        }

        let name = tokens[0].to_ascii_lowercase();
        let args = tokens[1..].iter().map(|value| value.to_string()).collect::<Vec<_>>();
        directives.push(ParsedDirective {
            name: name.clone(),
            args: args.clone(),
            line: line_no,
        });

        match name.as_str() {
            "remote" if !args.is_empty() => remotes.push(args.join(" ")),
            "dhcp-option" => dns_directives.push(args.join(" ")),
            "auth-user-pass" => {
                requires_auth_user_pass = true;
            }
            "ca" | "cert" | "key" | "pkcs12" | "tls-auth" | "tls-crypt" if !args.is_empty() => {
                let source_path = base_dir.join(&args[0]);
                let kind = AssetKind::from_directive(&name).unwrap_or(AssetKind::InlineBlob);
                referenced_assets.push(AssetReference {
                    directive: name.clone(),
                    kind,
                    source_path,
                    line: line_no,
                });
            }
            _ => {}
        }
    }

    Ok(ParsedProfile {
        directives,
        referenced_assets,
        inline_assets,
        remotes,
        dns_directives,
        requires_auth_user_pass,
    })
}

fn tokenize_line(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
            }
            ' ' | '\t' if !quoted => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::parse_profile;

    #[test]
    fn parses_relative_and_inline_assets() {
        let source = r#"
client
remote vpn.example.com 1194
ca ca.crt
auth-user-pass
<tls-crypt>
secret
</tls-crypt>
dhcp-option DNS 1.1.1.1
"#;

        let parsed = parse_profile(source, Path::new("/tmp/profile")).unwrap();
        assert_eq!(parsed.referenced_assets.len(), 1);
        assert_eq!(parsed.inline_assets.len(), 1);
        assert_eq!(parsed.remotes, vec!["vpn.example.com 1194"]);
        assert!(parsed.requires_auth_user_pass);
        assert_eq!(parsed.dns_directives, vec!["DNS 1.1.1.1"]);
    }
}

