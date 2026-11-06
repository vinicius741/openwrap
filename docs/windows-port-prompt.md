# Prompt: suporte a Windows no OpenWrap

> Para quem tem uma máquina Windows e quer portar o OpenWrap. Clone o repositório
> (`https://github.com/vinicius741/openwrap`), abra o Claude Code / Codex na raiz
> e cole o bloco abaixo (ou diga: "Leia `docs/windows-port-prompt.md` e siga o prompt").

---

Você vai adicionar suporte a **Windows** ao OpenWrap, um cliente desktop de perfis OpenVPN feito em Tauri 2 (React em `ui/`, Rust em `src-tauri/` e `crates/`). Hoje ele só funciona no macOS. Estou numa máquina Windows e posso rodar e testar tudo de verdade aqui.

## Antes de escrever código

1. Leia `CLAUDE.md`, `AGENTS.md`, `docs/architecture.md` e `docs/security-model.md`.
2. Confirme as observações abaixo lendo o código: elas vieram de uma análise feita no macOS, sem um Windows para testar. Se algo estiver errado, me diga.
3. Me mostre um plano curto da Fase 0 + Fase 1 e espere meu OK antes de começar. Pare e me consulte de novo ao fim de cada fase.

## Regras que não podem ser quebradas

- **O comportamento no macOS não pode mudar.** Todo código novo de Windows fica atrás de `#[cfg(windows)]` / `#[cfg(target_os = "windows")]`, e o que for de macOS continua atrás dos `cfg` atuais. Não remova o helper setuid nem o backend de Packet Tunnel.
- **Projeto hobby, sem custo:** nada que exija certificado pago de code signing. Um build sem assinatura (com aviso do SmartScreen) é aceitável.
- **Modelo de segurança:** `.ovpn` importado é entrada não confiável (a allowlist em `crates/openwrap-core/src/config/policy.rs` continua valendo); senhas **nunca** são persistidas, só o username; o arquivo temporário de auth precisa ser apagado depois do uso; os logs enviados à UI são sanitizados.
- Siga as convenções do repositório (nomes de arquivo, estilo, densidade de comentários). Use as traits `VpnBackend` e `SecretStore` (em `crates/openwrap-core/src/lib.rs`) como pontos de extensão em vez de espalhar `if windows` pelo core.
- Trabalhe numa branch própria (ex.: `feat/windows-support`), com commits pequenos por etapa.

## Pré-requisitos na máquina

- Rust stable (MSVC), Node 22, `cargo install tauri-cli --version "^2.0.0" --locked`, WebView2 (já vem no Windows 11).
- **OpenVPN Community 2.6+** instalado (https://openvpn.net/community-downloads/) com o **Interactive Service** e o driver (Wintun/DCO). O executável costuma ficar em `C:\Program Files\OpenVPN\bin\openvpn.exe`.
- Um perfil `.ovpn` de teste que funcione (ex.: o da empresa, ou um servidor de teste).

## O que a análise encontrou (verifique)

| Área | Hoje | O que fazer no Windows |
|---|---|---|
| Build | `crates/openwrap-helper` usa `std::os::unix`, `libc::getpwuid_r`, `tokio::signal::unix` sem `cfg`; o `beforeBuildCommand` do `src-tauri/tauri.conf.json` compila o helper e `bundle.resources` o empacota. O workspace provavelmente **não compila** no Windows. | Isolar o helper para unix (cfg no crate ou stub de `main` para não-unix) e ajustar build/resources do Tauri por plataforma (ex.: `tauri.windows.conf.json`). |
| Backend | `src-tauri/src/app_state/backend_factory.rs` escolhe o helper setuid no macOS e cai no `DirectOpenVpnBackend` nas outras plataformas. | Fase 1: `DirectOpenVpnBackend` com o app rodando como admin. Fase 2: novo backend pelo Interactive Service. |
| Disconnect | `helper_launcher.rs` e `direct_launcher.rs` rodam `/bin/kill -TERM <pid>`. | O Windows não tem SIGTERM. `TerminateProcess` (`child.start_kill()`) mata sem limpar rotas e DNS. Encerramento gracioso: `signal SIGTERM` pela management interface (Fase 2). |
| DNS | Scripts shell de macOS (`crates/openwrap-core/src/dns/macos/`) são injetados via `route-up`/`down` em `connection/manager/runtime.rs`, só com `cfg(target_os = "macos")`; o reconcile de DNS no startup também é só macOS. | **Não** portar os scripts. No Windows o próprio OpenVPN aplica `dhcp-option DNS` / a diretiva `dns` (split DNS via NRPT) pelo Interactive Service. Mapear `DnsPolicy` para isso e ajustar `default_effective_mode` em `dns/observer.rs` (hoje vira `ObserveOnly` fora do macOS). |
| Segredos | `secrets/keychain.rs` usa `security-framework`; fora do macOS o stub devolve erro, então "lembrar usuário" quebra. | Implementar com o Windows Credential Manager (crate `keyring` ou `windows`/`CredWriteW`), mantendo o service name `app.openwrap.credentials`. |
| Detecção do OpenVPN | `openvpn/runtime.rs` só procura caminhos do Homebrew/Linux. | Adicionar `C:\Program Files\OpenVPN\bin\openvpn.exe` (e ler o caminho de instalação do registro `HKLM\SOFTWARE\OpenVPN`, se for simples). |
| Pasta de dados | `src-tauri/src/lib.rs` usa `dirs::data_local_dir()/OpenWrap`, que vira `%LOCALAPPDATA%\OpenWrap`. Só o helper fixa `Library/Application Support`. | Provavelmente nada. Testar com nome de usuário **com espaço** (o `quote_openvpn_arg` escapa `\` e espaço). |
| Permissões | `0o600`/`0o700` atrás de `cfg(unix)` em `runtime.rs` e `secrets/local_db.rs`, com fallback no-op. | Aceitável para o MVP (`%LOCALAPPDATA%` já tem ACL por usuário). Opcional: endurecer a ACL do arquivo de auth. |
| Revelar logs | `src-tauri/src/commands/logs.rs` chama `/usr/bin/open`. | Adicionar o `tauri-plugin-opener` ou usar `explorer /select,`. |
| Updates | `src-tauri/src/commands/updates.rs` compara e instala o helper com `sudo install`. | No Windows, reportar o helper como não aplicável e esconder o comando de reinstalação na UI. |
| Policy | Allowlist em `config/policy.rs`. | Avaliar liberar `block-outside-dns`, `windows-driver`, `dns`. `management` continua bloqueado no perfil importado: o app adiciona a própria diretiva na config gerada. |
| UI | Textos citam "Finder", "Keychain", "macOS may ask for your password" (`ui/src/features/...`). | Textos por plataforma. Expor a plataforma via comando IPC ou `@tauri-apps/plugin-os`. |
| Testes | Testes do helper e do `direct_launcher` usam `std::os::unix::fs::PermissionsExt`. | Pôr `cfg(unix)` onde preciso para `cargo test -p openwrap-core -p openwrap-app` passar no Windows. |

## Fases

### Fase 0: compilar no Windows
- `cargo check -p openwrap-core -p openwrap-app` e `cargo test -p openwrap-core` passando no Windows.
- `npm install`, `npm test` e `npm run tauri:dev` abrindo a janela.

### Fase 1: MVP rodando como administrador
- Detecção do `openvpn.exe`, `DirectOpenVpnBackend` no Windows, DNS delegado ao OpenVPN, Credential Manager, revelar logs, textos da UI e status do helper.
- Disconnect: o melhor possível sem management (documente a limitação).
- O app precisa rodar elevado. Escolha entre exigir "Executar como administrador" ou embutir um manifesto `requireAdministrator`, e me explique o trade-off (UAC a cada abertura, autostart).

### Fase 2: sem admin, via Interactive Service (a versão "certa")
- Novo `VpnBackend` que inicia o OpenVPN pelo named pipe do Interactive Service (`\\.\pipe\openvpn\service`, em instalações padrão). A mensagem de startup é UTF-16 com três campos terminados em NUL: working dir, linha de opções, dados de stdin. O serviço responde com código de erro e PID. **Confirme o protocolo no código-fonte do openvpn-gui** (`openvpn.c`, função de start via serviço) e do `openvpnserv` antes de implementar. Não confie só nesta descrição.
- Como não há stdout, use a **management interface** (`--management 127.0.0.1 <porta> <arquivo-de-senha>`, `--management-hold`, `--management-query-passwords`) para: logs (`log on all`), estado (`state on`), envio de usuário/senha (sem gravar em disco) e disconnect gracioso (`signal SIGTERM`). Gere porta e senha aleatórias por sessão.
- Os logs e estados precisam chegar ao `ConnectionManager` no mesmo formato de `BackendEvent` usado hoje, para o `log_parser.rs` e a máquina de estados seguirem iguais.
- Risco conhecido: o Interactive Service restringe opções quando a config não está no diretório de config global e o usuário não está no grupo **"OpenVPN Administrators"**. Investigue e documente o que o usuário precisa fazer.

### Fase 3 (opcional, me pergunte antes): empacotamento e release
- Bundle NSIS/MSI (`bundle.targets` por plataforma) e plataforma `windows-x86_64` no updater.
- Job `windows-latest` em `.github/workflows/release.yml` **sem alterar** o job de macOS que já existe. Atualizar `scripts/package-release.sh` / geração do `latest.json` se necessário.

## Como validar (no Windows real)

Para cada fase, me mostre os comandos executados e os resultados:
- Importar um `.ovpn`, conectar, ver o estado chegar a **Connected** e os logs aparecerem na UI.
- `route print` e `Get-DnsClientServerAddress` / `Get-DnsClientNrptPolicy` durante a conexão e depois do disconnect: rotas e DNS **precisam voltar ao normal**.
- Disconnect pela UI, pelo tray e fechando o app no meio da conexão. Depois, conectar de novo.
- Credenciais erradas mostram o erro certo; "lembrar usuário" sobrevive a reiniciar o app.
- Usuário do Windows com espaço no nome.
- `cargo test -p openwrap-core -p openwrap-app` e `npm test` passando no Windows. Se der para checar, também não quebram no macOS (CI ou alguém com Mac).

## Entrega

- Branch com commits por fase e um PR descrevendo o que foi feito, o que foi testado (com saídas) e as limitações conhecidas.
- Um `docs/windows-setup.md` curto: pré-requisitos, instalação do OpenVPN, como rodar em dev e como usar.
