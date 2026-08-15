//! Running a package manager as somebody with more rights than the person at
//! the keyboard, and saying plainly when that was refused.
//!
//! Nothing here caches an authorisation. Whether a password is asked once or
//! every time is the elevator's decision: polkit's `org.freedesktop.policykit.exec`
//! action defaults to `auth_admin`, which authenticates each run, and only a
//! polkit rule granting `auth_admin_keep` changes that.

use std::process::{Command, ExitStatus};

/// Whose packages an operation is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackageMode {
    #[default]
    User,
    System,
}

impl std::fmt::Display for PackageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageMode::User => write!(f, "User"),
            PackageMode::System => write!(f, "System"),
        }
    }
}

/// A way of asking for more rights than the caller has. The `Display` name is
/// also the binary, which is what [`detect_elevator`] looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorType {
    Sudo,
    Doas,
    Pkexec,
}

impl std::fmt::Display for ElevatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElevatorType::Sudo => write!(f, "sudo"),
            ElevatorType::Doas => write!(f, "doas"),
            ElevatorType::Pkexec => write!(f, "pkexec"),
        }
    }
}

/// Which elevator to ask.
///
/// pkexec comes first because it is the only one that can put the question on
/// the screen. The others read a terminal, and there is none behind a window.
pub fn detect_elevator() -> Option<ElevatorType> {
    if which::which("pkexec").is_ok() {
        Some(ElevatorType::Pkexec)
    } else if which::which("sudo").is_ok() {
        Some(ElevatorType::Sudo)
    } else if which::which("doas").is_ok() {
        Some(ElevatorType::Doas)
    } else {
        None
    }
}

/// The same command, run by somebody who is allowed to run it, and whichever
/// elevator was asked so a refusal can be told apart from a real failure.
///
/// The environment is carried over for sudo and doas. pkexec keeps only a
/// short list of its own (PATH, HOME, SHELL, USER, LOGNAME, the locale, and
/// the X11 pair) and discards the rest, which is its whole point.
pub fn elevated(
    mode: PackageMode,
    cmd: Command,
) -> Result<(Command, Option<ElevatorType>), PrivilegeError> {
    if mode == PackageMode::User {
        return Ok((cmd, None));
    }

    let elevator = detect_elevator().ok_or(PrivilegeError::NoElevatorFound)?;

    let mut raised = Command::new(elevator.to_string());
    raised.arg(cmd.get_program()).args(cmd.get_args());

    if let Some(dir) = cmd.get_current_dir() {
        raised.current_dir(dir);
    }
    for (key, value) in cmd.get_envs() {
        match value {
            Some(value) => raised.env(key, value),
            None => raised.env_remove(key),
        };
    }

    Ok((raised, Some(elevator)))
}

/// pkexec's own exit codes, for the case where it goes without saying. Only a
/// dismissal has one of its own; everything else it refuses keeps the 127 it
/// starts with.
const DISMISSED: i32 = 126;
const NOT_AUTHORISED: i32 = 127;

const WAS_DISMISSED: &str = "the password prompt was dismissed";
const NO_AGENT: &str = "nothing was there to ask for a password. \
     A polkit authentication agent has to be running in this session";
const NOT_ALLOWED: &str = "not authorised to run this";
/// Where pkexec refused without saying which of the two it was.
const REFUSED: &str = "not authorised, and there may be no polkit authentication agent \
     running in this session to ask for a password";

/// Why the elevator refused, where the failure was its and not the manager's.
///
/// `None` means the manager itself ran and failed, and what it said about that
/// is the better thing to report.
pub fn refused(elevator: ElevatorType, status: ExitStatus, said: &str) -> Option<String> {
    match elevator {
        ElevatorType::Pkexec => pkexec_refused(status, said),
        ElevatorType::Sudo | ElevatorType::Doas => {
            let said = said.to_ascii_lowercase();
            (said.contains("no tty") || said.contains("askpass")).then(|| {
                format!(
                    "{elevator} cannot ask for a password from a window. \
                     Installing pkexec and a polkit authentication agent would let the desktop ask instead"
                )
            })
        }
    }
}

fn pkexec_refused(status: ExitStatus, said: &str) -> Option<String> {
    // pkexec says which of these it was, and says it on stderr as well as to
    // the log, so the wording is worth reading before the exit code.
    let lowered = said.to_ascii_lowercase();
    if lowered.contains("request dismissed") {
        return Some(WAS_DISMISSED.into());
    }
    if lowered.contains("no authentication agent") {
        return Some(NO_AGENT.into());
    }
    if lowered.contains("not authorized") || lowered.contains("not authorised") {
        return Some(NOT_ALLOWED.into());
    }

    // Read the exit code only when nothing was said. A manager can exit 126 or
    // 127 for reasons of its own, but it would have complained on the way out,
    // and by then pkexec is only passing that code along.
    if !said.trim().is_empty() {
        return None;
    }
    match status.code() {
        Some(DISMISSED) => Some(WAS_DISMISSED.into()),
        Some(NOT_AUTHORISED) => Some(REFUSED.into()),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrivilegeError {
    #[error("no way to ask for more rights was found. One of pkexec, sudo or doas has to be installed")]
    NoElevatorFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn exited(code: i32) -> ExitStatus {
        // The wait status packs the exit code into the high byte.
        ExitStatus::from_raw(code << 8)
    }

    #[test]
    fn a_dismissed_prompt_is_named_as_one() {
        let said = "Error executing command as another user: Request dismissed";
        assert_eq!(
            refused(ElevatorType::Pkexec, exited(DISMISSED), said).as_deref(),
            Some(WAS_DISMISSED)
        );
    }

    #[test]
    fn a_missing_agent_is_told_apart_from_being_turned_down() {
        // pkexec has a message of its own for each, and both leave it on 127.
        let none = "Error executing command as another user: No authentication agent found.";
        assert_eq!(
            refused(ElevatorType::Pkexec, exited(NOT_AUTHORISED), none).as_deref(),
            Some(NO_AGENT)
        );

        let denied = "Error executing command as another user: Not authorized";
        assert_eq!(
            refused(ElevatorType::Pkexec, exited(NOT_AUTHORISED), denied).as_deref(),
            Some(NOT_ALLOWED)
        );
    }

    #[test]
    fn silence_is_read_from_the_exit_code_alone() {
        assert_eq!(
            refused(ElevatorType::Pkexec, exited(DISMISSED), "").as_deref(),
            Some(WAS_DISMISSED)
        );
        // Nothing said means the two 127s cannot be separated, so say both.
        assert_eq!(
            refused(ElevatorType::Pkexec, exited(NOT_AUTHORISED), "  \n ").as_deref(),
            Some(REFUSED)
        );
    }

    #[test]
    fn a_manager_exiting_127_is_left_to_speak_for_itself() {
        // The code alone would say "not authorised", which would be wrong.
        let said = "pacstall: command not found";
        assert_eq!(
            refused(ElevatorType::Pkexec, exited(NOT_AUTHORISED), said),
            None
        );
    }

    #[test]
    fn an_ordinary_failure_is_not_blamed_on_the_elevator() {
        let said = "E: Unable to locate package nosuchthing";
        assert_eq!(refused(ElevatorType::Pkexec, exited(1), said), None);
        assert_eq!(refused(ElevatorType::Sudo, exited(1), said), None);
    }

    #[test]
    fn sudo_without_a_terminal_says_so() {
        let said = "sudo: no tty present and no askpass program specified";
        let why = refused(ElevatorType::Sudo, exited(1), said).expect("should be recognised");
        assert!(why.starts_with("sudo cannot ask for a password from a window"));
    }

    #[test]
    fn user_mode_is_left_alone() {
        let (cmd, elevator) = elevated(PackageMode::User, Command::new("pacstall")).unwrap();
        assert_eq!(cmd.get_program(), "pacstall");
        assert!(elevator.is_none());
    }

    #[test]
    fn elevating_keeps_the_arguments_apart() {
        let Some(expected) = detect_elevator() else {
            return;
        };

        let mut original = Command::new("pacstall");
        original.args(["-I", "a package with spaces"]);

        let (raised, used) = elevated(PackageMode::System, original).unwrap();
        assert_eq!(used, Some(expected));
        assert_eq!(raised.get_program(), expected.to_string().as_str());

        let args: Vec<_> = raised.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args, ["pacstall", "-I", "a package with spaces"]);
    }
}
