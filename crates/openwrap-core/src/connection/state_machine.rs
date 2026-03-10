use crate::connection::session::ConnectionState;
use crate::errors::AppError;

#[derive(Debug, Clone, Copy)]
pub enum ConnectionIntent {
    BeginConnect,
    NeedCredentials,
    CredentialsReady,
    PrepareRuntime,
    Spawned,
    Connected,
    Retry,
    BeginDisconnect,
    FinishDisconnect,
    Fatal,
    Reset,
}

pub fn transition(current: ConnectionState, intent: ConnectionIntent) -> Result<ConnectionState, AppError> {
    let next = match (current, intent) {
        (ConnectionState::Idle, ConnectionIntent::BeginConnect) => ConnectionState::ValidatingProfile,
        (ConnectionState::ValidatingProfile, ConnectionIntent::NeedCredentials) => {
            ConnectionState::AwaitingCredentials
        }
        (ConnectionState::ValidatingProfile, ConnectionIntent::PrepareRuntime)
        | (ConnectionState::AwaitingCredentials, ConnectionIntent::CredentialsReady) => {
            ConnectionState::PreparingRuntime
        }
        (ConnectionState::PreparingRuntime, ConnectionIntent::Spawned) => ConnectionState::StartingProcess,
        (ConnectionState::StartingProcess, ConnectionIntent::PrepareRuntime) => ConnectionState::Connecting,
        (ConnectionState::Connecting, ConnectionIntent::Connected) => ConnectionState::Connected,
        (ConnectionState::Connecting, ConnectionIntent::Retry)
        | (ConnectionState::Connected, ConnectionIntent::Retry) => ConnectionState::Reconnecting,
        (ConnectionState::Reconnecting, ConnectionIntent::Spawned) => ConnectionState::StartingProcess,
        (ConnectionState::Connecting, ConnectionIntent::BeginDisconnect)
        | (ConnectionState::Connected, ConnectionIntent::BeginDisconnect)
        | (ConnectionState::Reconnecting, ConnectionIntent::BeginDisconnect)
        | (ConnectionState::StartingProcess, ConnectionIntent::BeginDisconnect)
        | (ConnectionState::PreparingRuntime, ConnectionIntent::BeginDisconnect) => {
            ConnectionState::Disconnecting
        }
        (ConnectionState::Disconnecting, ConnectionIntent::FinishDisconnect)
        | (ConnectionState::Error, ConnectionIntent::Reset) => ConnectionState::Idle,
        (_, ConnectionIntent::Fatal) => ConnectionState::Error,
        (state, intent) => {
            return Err(AppError::ConnectionState(format!(
                "invalid transition from {state:?} with {intent:?}"
            )))
        }
    };

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::{transition, ConnectionIntent};
    use crate::connection::ConnectionState;

    #[test]
    fn walks_through_success_path() {
        let validating = transition(ConnectionState::Idle, ConnectionIntent::BeginConnect).unwrap();
        let preparing = transition(validating, ConnectionIntent::PrepareRuntime).unwrap();
        let starting = transition(preparing, ConnectionIntent::Spawned).unwrap();
        let connecting = transition(starting, ConnectionIntent::PrepareRuntime).unwrap();
        let connected = transition(connecting, ConnectionIntent::Connected).unwrap();
        assert_eq!(connected, ConnectionState::Connected);
    }

    #[test]
    fn handles_credential_path() {
        let validating = transition(ConnectionState::Idle, ConnectionIntent::BeginConnect).unwrap();
        let awaiting = transition(validating, ConnectionIntent::NeedCredentials).unwrap();
        let preparing = transition(awaiting, ConnectionIntent::CredentialsReady).unwrap();
        assert_eq!(preparing, ConnectionState::PreparingRuntime);
    }

    #[test]
    fn rejects_invalid_transition() {
        assert!(transition(ConnectionState::Idle, ConnectionIntent::Connected).is_err());
    }
}

