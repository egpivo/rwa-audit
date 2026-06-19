//! CLI integration tests for unified rwa-audit binary.

use rwa_audit::audit::{parse_freeze_command, parse_run_command, FreezeAction};

#[test]
fn run_registry_live_mode() {
    let mut args = vec!["registry".into(), "--mode".into(), "live".into()];
    let mut iter = args.drain(..);
    let cmd = parse_run_command(&mut iter).unwrap();
    assert_eq!(cmd.module, "registry");
    assert!(cmd.mode.is_live());
}

#[test]
fn freeze_exchange_flags() {
    let mut args = vec!["exchange".into(), "--live".into(), "--refresh-rwa".into()];
    let mut iter = args.drain(..);
    let cmd = parse_freeze_command(&mut iter).unwrap();
    match cmd.action {
        FreezeAction::Exchange { live, refresh_rwa } => {
            assert!(live);
            assert!(refresh_rwa);
        }
        _ => panic!("expected exchange freeze"),
    }
}
