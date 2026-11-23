use std::collections::HashMap;

use crate::profiles::{AssetKind, ParsedProfile};

pub fn rewrite_profile(
    parsed: &ParsedProfile,
    rewritten_assets: &HashMap<AssetKind, String>,
) -> String {
    let mut output = Vec::new();
    let mut auth_nocache_present = false;

    for directive in &parsed.directives {
        if directive.name == "auth-nocache" {
            auth_nocache_present = true;
        }

        if let Some(kind) = AssetKind::from_directive(&directive.name) {
            if let Some(path) = rewritten_assets.get(&kind) {
                let mut parts = vec![directive.name.clone(), path.clone()];
                parts.extend(directive.args.iter().skip(1).map(|arg| quote_arg(arg)));
                output.push(parts.join(" "));
                continue;
            }
        }

        if directive.name == "auth-user-pass" {
            output.push("auth-user-pass".to_string());
            continue;
        }

        let mut parts = vec![directive.name.clone()];
        parts.extend(directive.args.iter().map(|arg| quote_arg(arg)));
        output.push(parts.join(" "));
    }

    for inline in &parsed.inline_assets {
        if let Some(path) = rewritten_assets.get(&inline.kind) {
            output.push(format!("{} {}", inline.directive, path));
        }
    }

    if !auth_nocache_present {
        output.push("auth-nocache".to_string());
    }

    output.join("\n") + "\n"
}

/// Restores the quotes the tokenizer removed, so an argument with spaces
/// (e.g. `verify-x509-name "VPN Server" name`) stays one argument.
///
/// The tokenizer keeps backslashes as written, so they are written back
/// unchanged; only arguments that need quotes get them.
fn quote_arg(arg: &str) -> String {
    if arg.is_empty() || arg.chars().any(char::is_whitespace) {
        format!("\"{arg}\"")
    } else {
        arg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use crate::config::parse_profile;
    use crate::profiles::AssetKind;

    use super::rewrite_profile;

    #[test]
    fn preserves_extra_arguments_for_asset_directives() {
        let parsed = parse_profile("tls-auth ta.key 1\n", Path::new("/tmp")).unwrap();
        let rewritten = rewrite_profile(
            &parsed,
            &HashMap::from([(AssetKind::TlsAuth, "assets/tls-auth.key".to_string())]),
        );

        assert_eq!(rewritten, "tls-auth assets/tls-auth.key 1\nauth-nocache\n");
    }

    #[test]
    fn keeps_quoted_arguments_with_spaces_together() {
        let parsed = parse_profile(
            "verify-x509-name \"VPN Server CN\" name\nremote vpn.example.com 1194\n",
            Path::new("/tmp"),
        )
        .unwrap();
        let rewritten = rewrite_profile(&parsed, &HashMap::new());

        assert_eq!(
            rewritten,
            "verify-x509-name \"VPN Server CN\" name\nremote vpn.example.com 1194\nauth-nocache\n"
        );
        // And it parses back to the same arguments.
        let reparsed = parse_profile(&rewritten, Path::new("/tmp")).unwrap();
        assert_eq!(reparsed.directives[0].args, vec!["VPN Server CN", "name"]);
    }
}
